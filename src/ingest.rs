//! Ingest: the four jobs that copy NewsWeb into the store and the files.
//!
//! - `poller` lists today, once every poll interval.
//! - `backfiller` lists the past days of the lookback range that are not
//!   sealed, newest first, and records each listing in `listed_days`.
//! - `reader` reads in full the disclosures that the lists stored.
//! - `downloader` downloads the attachments of the disclosures read in full.
//!
//! Every day is an Oslo calendar day. A day is *sealed*, known complete, by a
//! listing of it that started at least `SEALING_GRACE_NS` after the day ended
//! and was neither truncated nor had undecodable rows.
//!
//! The decisions are pure functions at the top of this file; the jobs are the
//! I/O around them. Each job is a pass, which does one round of work, and a
//! loop that repeats the pass until a `CancellationToken` is cancelled.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::time::Duration;

use jiff::Span;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::clock::Clock;
use crate::exchange::client::{ClientError, Exchange};
use crate::exchange::{self, Fetched, Page};
use crate::files::Files;
use crate::store::{AttachmentRef, Failure, ListedDay, Store, StoreError, StoredFile};

/// The names of the jobs, as the logs give them.
pub const POLLER: &str = "poller";
pub const BACKFILLER: &str = "backfiller";
pub const READER: &str = "reader";
pub const DOWNLOADER: &str = "downloader";

const HOUR_NS: i64 = 3_600_000_000_000;

/// How long after the end of a day a listing of it must start to seal it.
/// The exchange was measured to show a disclosure up to an hour after its
/// publication time.
pub const SEALING_GRACE_NS: i64 = 2 * HOUR_NS;

/// How old a listing that started after the day's sealing time, and still
/// did not seal it, must be before the day is listed again.
pub const RELIST_AFTER_NS: i64 = 24 * HOUR_NS;

/// The disclosures, or attachments, that one pass of `reader`, or
/// `downloader`, takes from its queue.
pub const BATCH_SIZE: u32 = 20;

/// The instant from which a listing of `day` can seal it: the next Oslo
/// midnight after `day`, plus `SEALING_GRACE_NS`. `i64::MAX` for a day that
/// ends beyond the range of i64 nanoseconds.
pub fn sealing_time_ns(day: Date) -> i64 {
    day.tomorrow()
        .and_then(|next| next.to_zoned(oslo()))
        .ok()
        .and_then(|midnight| i64::try_from(midnight.timestamp().as_nanosecond()).ok())
        .map_or(i64::MAX, |midnight_ns| {
            midnight_ns.saturating_add(SEALING_GRACE_NS)
        })
}

/// Whether a complete listing of `day` that started at `listed_at_ns` seals
/// the day.
pub fn seals(day: Date, listed_at_ns: i64) -> bool {
    listed_at_ns >= sealing_time_ns(day)
}

/// The record of a listing of `day` that started at `listed_at_ns` and
/// returned `page`. A truncated list, or one with undecodable rows, does not
/// seal the day, whenever it ran.
pub fn listed_day(day: Date, listed_at_ns: i64, page: &Page) -> ListedDay {
    let entries = u32::try_from(page.entries.len()).unwrap_or(u32::MAX);
    let complete = !page.overflow && page.undecodable == 0;
    ListedDay {
        day,
        listed_at_ns,
        rows: entries.saturating_add(page.undecodable),
        overflow: page.overflow,
        undecodable: page.undecodable,
        sealed: complete && seals(day, listed_at_ns),
    }
}

/// What makes a listing incomplete, as the failure log states it; `None`
/// for a complete listing.
pub fn incomplete_listing(page: &Page) -> Option<String> {
    let undecodable = page.undecodable;
    match (page.overflow, undecodable) {
        (false, 0) => None,
        (true, 0) => Some("the exchange truncated the list".to_owned()),
        (false, _) => Some(format!("undecodable rows on the list: {undecodable}")),
        (true, _) => Some(format!(
            "the exchange truncated the list; undecodable rows on it: {undecodable}"
        )),
    }
}

/// Consecutive days, both ends included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayRange {
    pub first: Date,
    pub last: Date,
}

impl DayRange {
    /// The days of the range, ascending; no day when `last` is before `first`.
    pub fn days(self) -> impl Iterator<Item = Date> {
        self.first
            .series(Span::new().days(1))
            .take_while(move |day| *day <= self.last)
    }
}

impl fmt::Display for DayRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.first == self.last {
            write!(f, "{}", self.first)
        } else {
            write!(f, "{}..{}", self.first, self.last)
        }
    }
}

/// Which past days of the lookback range are sealed, and which are gaps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    /// The first day of the range; the last is yesterday.
    pub first_day: Date,
    /// The sealed days, as ranges in ascending order.
    pub sealed: Vec<DayRange>,
    /// The days not sealed whose sealing time has passed, as ranges in
    /// ascending order.
    pub gaps: Vec<DayRange>,
}

impl Coverage {
    /// The number of days in the gaps.
    pub fn gap_days(&self) -> usize {
        self.gaps.iter().map(|gap| gap.days().count()).sum()
    }
}

/// The lookback range at `now_ns`: from `lookback_days` before today to
/// yesterday. Today is never in it. Empty when `lookback_days` is 0.
pub fn lookback_range(now_ns: i64, lookback_days: u32) -> DayRange {
    let today = exchange::oslo_day(now_ns);
    DayRange {
        first: days_before(today, lookback_days),
        last: days_before(today, 1),
    }
}

/// The coverage of the lookback range at `now_ns`, from the listings of its
/// days. A day not sealed is a gap once its sealing time has passed, so
/// yesterday is not a gap within the grace.
pub fn coverage(listed: &[ListedDay], now_ns: i64, lookback_days: u32) -> Coverage {
    let range = lookback_range(now_ns, lookback_days);
    let sealed: HashSet<Date> = listed
        .iter()
        .filter(|listing| listing.sealed)
        .map(|listing| listing.day)
        .collect();
    let sealed_days = range.days().filter(|day| sealed.contains(day));
    let gap_days = range
        .days()
        .filter(|day| !sealed.contains(day) && sealing_time_ns(*day) <= now_ns);
    Coverage {
        first_day: range.first,
        sealed: ranges(sealed_days),
        gaps: ranges(gap_days),
    }
}

/// The gap days to list now, newest first. A day whose latest listing
/// started after its sealing time and still did not seal it is left out
/// until that listing is `RELIST_AFTER_NS` old, so a day that cannot seal
/// costs one request a day.
pub fn backfill_plan(listed: &[ListedDay], now_ns: i64, lookback_days: u32) -> Vec<Date> {
    let latest: HashMap<Date, &ListedDay> = listed
        .iter()
        .map(|listing| (listing.day, listing))
        .collect();
    let mut plan: Vec<Date> = coverage(listed, now_ns, lookback_days)
        .gaps
        .iter()
        .flat_map(|gap| gap.days())
        .filter(|day| due(latest.get(day).copied(), now_ns))
        .collect();
    plan.reverse();
    plan
}

/// How a failed call to the exchange is logged in the store.
pub fn failure(error: &ClientError) -> Failure {
    Failure {
        permanent: error.is_permanent(),
        detail: error.to_string(),
    }
}

/// The ranges, separated by commas; "none" when there is none.
pub fn format_ranges(ranges: &[DayRange]) -> String {
    if ranges.is_empty() {
        return "none".to_owned();
    }
    ranges
        .iter()
        .map(DayRange::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Whether a gap day is listed now, given its latest listing.
fn due(latest: Option<&ListedDay>, now_ns: i64) -> bool {
    match latest {
        Some(listing) if listing.listed_at_ns >= sealing_time_ns(listing.day) => {
            now_ns.saturating_sub(listing.listed_at_ns) > RELIST_AFTER_NS
        }
        _ => true,
    }
}

/// Days in ascending order, with consecutive days joined into one range.
fn ranges(days: impl Iterator<Item = Date>) -> Vec<DayRange> {
    let mut ranges: Vec<DayRange> = Vec::new();
    for day in days {
        match ranges.last_mut() {
            Some(range) if range.last.tomorrow().is_ok_and(|next| next == day) => {
                range.last = day;
            }
            _ => ranges.push(DayRange {
                first: day,
                last: day,
            }),
        }
    }
    ranges
}

fn days_before(day: Date, days: u32) -> Date {
    Span::new()
        .try_days(i64::from(days))
        .and_then(|span| day.checked_sub(span))
        .unwrap_or(Date::MIN)
}

fn oslo() -> TimeZone {
    TimeZone::get("Europe/Oslo").expect("the bundled time zone database has Oslo")
}

/// Logs which days of the lookback range are sealed and which are gaps.
pub async fn log_coverage(
    store: &Store,
    now_ns: i64,
    lookback_days: u32,
) -> Result<(), StoreError> {
    let range = lookback_range(now_ns, lookback_days);
    let listed = store.listed_days(range.first, range.last).await?;
    let coverage = coverage(&listed, now_ns, lookback_days);
    let from = coverage.first_day;
    let sealed = format_ranges(&coverage.sealed);
    match (coverage.gaps.first(), coverage.gaps.last()) {
        (Some(first), Some(last)) => tracing::info!(
            %from,
            %sealed,
            gap_days = coverage.gap_days(),
            first_gap = %first.first,
            last_gap = %last.last,
            "coverage"
        ),
        _ => tracing::info!(%from, %sealed, "coverage: no gap"),
    }
    Ok(())
}

/// The settings of the jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobSettings {
    /// How often `poller` lists today, and how long the other jobs wait
    /// after a pass that found no work.
    pub poll_interval: Duration,
    /// How many past days `backfiller` keeps listed.
    pub lookback_days: u32,
}

/// The four jobs and what they work with.
pub struct Jobs<'a, E, C> {
    pub store: &'a Store,
    pub files: &'a Files,
    pub exchange: &'a E,
    pub clock: &'a C,
}

impl<E: Exchange, C: Clock> Jobs<'_, E, C> {
    /// Runs the four jobs until `shutdown` is cancelled.
    pub async fn run(&self, settings: &JobSettings, shutdown: &CancellationToken) {
        let interval = settings.poll_interval;
        tokio::join!(
            self.run_poller(interval, shutdown),
            self.run_backfiller(settings, shutdown),
            self.run_reader(interval, shutdown),
            self.run_downloader(interval, shutdown),
        );
    }

    /// Runs a pass of `poller` once every `interval` until `shutdown` is
    /// cancelled. A failed pass does not delay the next one.
    pub async fn run_poller(&self, interval: Duration, shutdown: &CancellationToken) {
        loop {
            let next = Instant::now() + interval;
            let Some(result) = shutdown.run_until_cancelled(self.poller_pass()).await else {
                return;
            };
            if let Err(error) = result {
                log_store_error(POLLER, &error);
            }
            let waited = shutdown
                .run_until_cancelled(tokio::time::sleep_until(next))
                .await;
            if waited.is_none() {
                return;
            }
        }
    }

    /// Runs passes of `backfiller` until `shutdown` is cancelled.
    pub async fn run_backfiller(&self, settings: &JobSettings, shutdown: &CancellationToken) {
        repeat(BACKFILLER, settings.poll_interval, shutdown, || {
            self.backfiller_pass(settings.lookback_days)
        })
        .await;
    }

    /// Runs passes of `reader` until `shutdown` is cancelled.
    pub async fn run_reader(&self, interval: Duration, shutdown: &CancellationToken) {
        repeat(READER, interval, shutdown, || self.reader_pass()).await;
    }

    /// Runs passes of `downloader` until `shutdown` is cancelled.
    pub async fn run_downloader(&self, interval: Duration, shutdown: &CancellationToken) {
        repeat(DOWNLOADER, interval, shutdown, || self.downloader_pass()).await;
    }

    /// Lists today with one request and stores every entry. It lists no
    /// other day and writes no row of `listed_days`: a listing of today
    /// cannot seal it. A failure to list is logged in the store.
    pub async fn poller_pass(&self) -> Result<(), StoreError> {
        let today = exchange::oslo_day(self.clock.now_ns());
        let page = match self.exchange.list(today).await {
            Ok(page) => page,
            Err(error) => {
                tracing::warn!(job = POLLER, day = %today, %error, "listing failed");
                let now_ns = self.clock.now_ns();
                return self
                    .store
                    .record_list_failure(today, &error.to_string(), now_ns)
                    .await;
            }
        };
        if let Some(detail) = incomplete_listing(&page) {
            tracing::warn!(job = POLLER, day = %today, detail, "listing incomplete");
        }
        let new = self.store_entries(&page.entries).await?;
        if new > 0 {
            tracing::info!(
                job = POLLER,
                day = %today,
                rows = page.entries.len(),
                new,
                "listed today"
            );
        }
        Ok(())
    }

    /// Lists each day of `backfill_plan`, stores every entry and records
    /// the listing in `listed_days`. A failure to list a day is logged in
    /// the store and ends the pass: the next day would most likely fail the
    /// same way, and a long plan would otherwise send a failing request for
    /// each of its days. Returns the number of days listed.
    pub async fn backfiller_pass(&self, lookback_days: u32) -> Result<usize, StoreError> {
        let now_ns = self.clock.now_ns();
        let range = lookback_range(now_ns, lookback_days);
        let listed = self.store.listed_days(range.first, range.last).await?;
        let plan = backfill_plan(&listed, now_ns, lookback_days);

        let (mut days, mut sealed, mut rows, mut new) = (0, 0, 0, 0);
        for day in plan {
            let Some(listing) = self.backfill_day(day).await? else {
                break;
            };
            days += 1;
            sealed += usize::from(listing.day.sealed);
            rows += listing.day.rows;
            new += listing.new;
        }
        if days > 0 {
            tracing::info!(
                job = BACKFILLER,
                days,
                sealed,
                rows,
                new,
                "listed past days"
            );
        }
        Ok(days)
    }

    /// Lists one day for `backfiller`. `None` when the listing failed.
    async fn backfill_day(&self, day: Date) -> Result<Option<Backfilled>, StoreError> {
        let started_at_ns = self.clock.now_ns();
        let page = match self.exchange.list(day).await {
            Ok(page) => page,
            Err(error) => {
                tracing::warn!(job = BACKFILLER, %day, %error, "listing failed");
                let now_ns = self.clock.now_ns();
                self.store
                    .record_list_failure(day, &error.to_string(), now_ns)
                    .await?;
                return Ok(None);
            }
        };
        let new = self.store_entries(&page.entries).await?;
        if let Some(detail) = incomplete_listing(&page) {
            tracing::warn!(job = BACKFILLER, %day, detail, "listing incomplete");
            let now_ns = self.clock.now_ns();
            self.store.record_list_failure(day, &detail, now_ns).await?;
        }
        let listing = listed_day(day, started_at_ns, &page);
        self.store.record_listed_day(&listing).await?;
        Ok(Some(Backfilled { day: listing, new }))
    }

    /// Stores rows of a list. Returns how many were new to the store.
    async fn store_entries(&self, entries: &[Fetched]) -> Result<usize, StoreError> {
        let mut new = 0;
        for entry in entries {
            let now_ns = self.clock.now_ns();
            new += usize::from(self.store.store_list_entry(entry, now_ns).await?);
        }
        Ok(new)
    }

    /// Reads in full up to `BATCH_SIZE` disclosures of the store's queue.
    /// A failed read is logged in the store, which delays the next try.
    /// Returns the number of disclosures tried.
    pub async fn reader_pass(&self) -> Result<usize, StoreError> {
        let now_ns = self.clock.now_ns();
        let message_ids = self.store.unread_disclosures(now_ns, BATCH_SIZE).await?;
        let mut failed = 0;
        for &message_id in &message_ids {
            match self.read(message_id).await {
                Ok(message) => {
                    let now_ns = self.clock.now_ns();
                    self.store.store_message(&message, now_ns).await?;
                }
                Err(failure) => {
                    failed += 1;
                    tracing::warn!(
                        job = READER,
                        message_id,
                        permanent = failure.permanent,
                        detail = failure.detail,
                        "reading failed"
                    );
                    let now_ns = self.clock.now_ns();
                    self.store
                        .record_read_failure(message_id, &failure, now_ns)
                        .await?;
                }
            }
        }
        let tried = message_ids.len();
        if tried > 0 {
            let read = tried - failed;
            tracing::info!(job = READER, tried, read, failed, "read disclosures");
        }
        Ok(tried)
    }

    async fn read(&self, message_id: i64) -> Result<Fetched, Failure> {
        let message = self
            .exchange
            .message(message_id)
            .await
            .map_err(|error| failure(&error))?;
        // Stored under another id, the message would leave this one in the
        // queue with no delay before the next try.
        let answered = message.disclosure.message_id;
        if answered != message_id {
            return Err(Failure {
                permanent: false,
                detail: format!("the exchange answered with message {answered}"),
            });
        }
        Ok(message)
    }

    /// Downloads up to `BATCH_SIZE` attachments of the store's queue. A
    /// failed download is logged in the store, which delays the next try; a
    /// failure to write the file is transient. Returns the number of
    /// attachments tried.
    pub async fn downloader_pass(&self) -> Result<usize, StoreError> {
        let now_ns = self.clock.now_ns();
        let pending = self.store.pending_attachments(now_ns, BATCH_SIZE).await?;
        let (mut failed, mut bytes) = (0, 0);
        for &AttachmentRef {
            message_id,
            attachment_id,
        } in &pending
        {
            match self.download(message_id, attachment_id).await {
                Ok(file) => {
                    bytes += file.size_bytes;
                    let now_ns = self.clock.now_ns();
                    self.store
                        .mark_attachment_stored(message_id, attachment_id, &file, now_ns)
                        .await?;
                }
                Err(failure) => {
                    failed += 1;
                    tracing::warn!(
                        job = DOWNLOADER,
                        message_id,
                        attachment_id,
                        permanent = failure.permanent,
                        detail = failure.detail,
                        "download failed"
                    );
                    let now_ns = self.clock.now_ns();
                    self.store
                        .record_attachment_failure(message_id, attachment_id, &failure, now_ns)
                        .await?;
                }
            }
        }
        let tried = pending.len();
        if tried > 0 {
            let stored = tried - failed;
            tracing::info!(
                job = DOWNLOADER,
                tried,
                stored,
                failed,
                bytes,
                "downloaded attachments"
            );
        }
        Ok(tried)
    }

    async fn download(&self, message_id: i64, attachment_id: i64) -> Result<StoredFile, Failure> {
        let bytes = self
            .exchange
            .attachment(message_id, attachment_id)
            .await
            .map_err(|error| failure(&error))?;
        self.files
            .put(message_id, attachment_id, bytes)
            .await
            .map_err(|error| Failure {
                permanent: false,
                detail: error.to_string(),
            })
    }
}

/// One day that `backfiller` listed.
struct Backfilled {
    day: ListedDay,
    /// Rows of the list that were new to the store.
    new: usize,
}

/// Repeats `pass` until `shutdown` is cancelled: at once after a pass that
/// did work, after `interval` when a pass found none or failed. A pass in
/// progress is dropped at cancellation; every write of the store is one
/// transaction, so a dropped pass leaves the store consistent.
async fn repeat<F, Pass>(job: &str, interval: Duration, shutdown: &CancellationToken, mut pass: F)
where
    F: FnMut() -> Pass,
    Pass: Future<Output = Result<usize, StoreError>>,
{
    loop {
        let Some(result) = shutdown.run_until_cancelled(pass()).await else {
            return;
        };
        let did_work = match result {
            Ok(done) => done > 0,
            Err(error) => {
                log_store_error(job, &error);
                false
            }
        };
        if did_work {
            continue;
        }
        let waited = shutdown
            .run_until_cancelled(tokio::time::sleep(interval))
            .await;
        if waited.is_none() {
            return;
        }
    }
}

fn log_store_error(job: &str, error: &StoreError) {
    tracing::warn!(job, %error, "store error; the job waits one poll interval");
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use crate::exchange::client::TransportError;

    use super::*;

    const HOUR: i64 = HOUR_NS;
    const MINUTE: i64 = HOUR_NS / 60;

    /// An instant given as Oslo local time, such as `2026-09-23T10:00`.
    fn oslo_ns(local: &str) -> i64 {
        let zoned: jiff::Zoned = format!("{local}[Europe/Oslo]").parse().unwrap();
        zoned.timestamp().as_nanosecond() as i64
    }

    /// An instant given in UTC, such as `2026-10-26T01:00:00Z`.
    fn utc_ns(utc: &str) -> i64 {
        let timestamp: jiff::Timestamp = utc.parse().unwrap();
        timestamp.as_nanosecond() as i64
    }

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    fn range(first: &str, last: &str) -> DayRange {
        DayRange {
            first: day(first),
            last: day(last),
        }
    }

    fn sealed_listing(day_text: &str) -> ListedDay {
        let day = day(day_text);
        ListedDay {
            day,
            listed_at_ns: sealing_time_ns(day),
            rows: 10,
            overflow: false,
            undecodable: 0,
            sealed: true,
        }
    }

    fn overflowed_listing(day_text: &str, listed_at_ns: i64) -> ListedDay {
        ListedDay {
            day: day(day_text),
            listed_at_ns,
            rows: 1_000,
            overflow: true,
            undecodable: 0,
            sealed: false,
        }
    }

    fn sealed_listings(first: &str, last: &str) -> Vec<ListedDay> {
        range(first, last)
            .days()
            .map(|day| sealed_listing(&day.to_string()))
            .collect()
    }

    fn page(entries: usize, overflow: bool, undecodable: u32) -> Page {
        let entry = Fetched {
            disclosure: Default::default(),
            document: "{}".to_owned(),
        };
        Page {
            entries: vec![entry; entries],
            overflow,
            undecodable,
        }
    }

    /// 10:00 in Oslo on 2026-09-23, a day of 24 hours.
    fn now() -> i64 {
        oslo_ns("2026-09-23T10:00")
    }

    #[test]
    fn seals__listed_at_the_next_midnight_plus_grace__returns_true() {
        let listed_at_ns = oslo_ns("2026-09-23T02:00");

        let sealed = seals(day("2026-09-22"), listed_at_ns);

        assert!(sealed);
    }

    #[test]
    fn seals__listed_one_minute_before_the_next_midnight_plus_grace__returns_false() {
        let listed_at_ns = oslo_ns("2026-09-23T01:59");

        let sealed = seals(day("2026-09-22"), listed_at_ns);

        assert!(!sealed);
    }

    #[test]
    fn seals__days_of_daylight_saving_changes__seal_two_hours_after_the_next_oslo_midnight() {
        // 2026-10-25 has 25 hours: its next midnight is 23:00 UTC. 2026-03-29
        // has 23 hours: its next midnight is 22:00 UTC.
        let cases = [
            ("2026-10-25", "2026-10-26T00:59:00Z", false),
            ("2026-10-25", "2026-10-26T01:00:00Z", true),
            ("2026-03-29", "2026-03-29T23:59:00Z", false),
            ("2026-03-29", "2026-03-30T00:00:00Z", true),
        ];
        for (day_text, listed_at, expected) in cases {
            assert_eq!(
                seals(day(day_text), utc_ns(listed_at)),
                expected,
                "day: {day_text}, listed at: {listed_at}"
            );
        }
    }

    #[test]
    fn sealing_time_ns__last_representable_day__returns_i64_max() {
        let sealing_time = sealing_time_ns(Date::MAX);

        assert_eq!(sealing_time, i64::MAX);
    }

    #[test]
    fn listed_day__complete_or_not__seals_only_a_complete_listing() {
        let listed_at_ns = oslo_ns("2026-09-23T10:00");
        let cases = [
            (page(3, false, 0), true),
            (page(3, true, 0), false),
            (page(3, false, 1), false),
            (page(3, true, 2), false),
        ];
        for (page, expected) in cases {
            assert_eq!(
                listed_day(day("2026-09-22"), listed_at_ns, &page).sealed,
                expected,
                "overflow: {}, undecodable: {}",
                page.overflow,
                page.undecodable
            );
        }
    }

    #[test]
    fn listed_day__undecodable_rows__counts_them_in_rows() {
        let listing = listed_day(day("2026-09-22"), now(), &page(3, false, 2));

        assert_eq!(listing.rows, 5);
    }

    #[test]
    fn incomplete_listing__overflow_and_undecodable_rows__names_what_is_missing() {
        let cases = [
            (page(3, false, 0), None),
            (
                page(3, true, 0),
                Some("the exchange truncated the list".to_owned()),
            ),
            (
                page(3, false, 2),
                Some("undecodable rows on the list: 2".to_owned()),
            ),
            (
                page(3, true, 2),
                Some("the exchange truncated the list; undecodable rows on it: 2".to_owned()),
            ),
        ];
        for (page, expected) in cases {
            assert_eq!(
                incomplete_listing(&page),
                expected,
                "overflow: {}, undecodable: {}",
                page.overflow,
                page.undecodable
            );
        }
    }

    #[test]
    fn lookback_range__seven_days__ends_yesterday() {
        let range = lookback_range(now(), 7);

        assert_eq!(range, self::range("2026-09-16", "2026-09-22"));
    }

    #[test]
    fn coverage__trailing_outage__reports_the_days_since_as_gaps() {
        let listed = sealed_listings("2026-09-16", "2026-09-19");

        let coverage = coverage(&listed, now(), 7);

        assert_eq!(coverage.gaps, [range("2026-09-20", "2026-09-22")]);
    }

    #[test]
    fn coverage__yesterday_within_the_grace__is_not_a_gap() {
        let listed = sealed_listings("2026-09-20", "2026-09-21");
        let within_grace = oslo_ns("2026-09-23T01:00");

        let coverage = coverage(&listed, within_grace, 3);

        assert_eq!(coverage.gaps, []);
    }

    #[test]
    fn coverage__gap_between_two_sealed_ranges__reports_the_gap() {
        let mut listed = sealed_listings("2026-09-16", "2026-09-17");
        listed.extend(sealed_listings("2026-09-20", "2026-09-22"));

        let coverage = coverage(&listed, now(), 7);

        assert_eq!(coverage.gaps, [range("2026-09-18", "2026-09-19")]);
    }

    #[test]
    fn coverage__sealed_days__reports_them_as_ranges() {
        let mut listed = sealed_listings("2026-09-16", "2026-09-17");
        listed.extend(sealed_listings("2026-09-20", "2026-09-22"));

        let coverage = coverage(&listed, now(), 7);

        assert_eq!(
            coverage.sealed,
            [
                range("2026-09-16", "2026-09-17"),
                range("2026-09-20", "2026-09-22")
            ]
        );
    }

    #[test]
    fn coverage__nothing_listed__reports_the_whole_range_as_one_gap() {
        let coverage = coverage(&[], now(), 7);

        assert_eq!(coverage.gaps, [range("2026-09-16", "2026-09-22")]);
    }

    #[test]
    fn coverage__sealed_day_before_the_range__is_ignored() {
        let listed = [sealed_listing("2026-09-15")];

        let coverage = coverage(&listed, now(), 7);

        assert_eq!(coverage.sealed, []);
    }

    #[test]
    fn coverage__unsealed_listing__leaves_the_day_a_gap() {
        let listed = [overflowed_listing("2026-09-22", now() - HOUR)];

        let coverage = coverage(&listed, now(), 1);

        assert_eq!(coverage.gaps, [range("2026-09-22", "2026-09-22")]);
    }

    #[test]
    fn coverage__gaps_of_three_and_two_days__counts_five_gap_days() {
        let listed = sealed_listings("2026-09-19", "2026-09-20");

        let coverage = coverage(&listed, now(), 7);

        assert_eq!(coverage.gap_days(), 5);
    }

    #[test]
    fn backfill_plan__nothing_listed__lists_the_newest_day_first() {
        let plan = backfill_plan(&[], now(), 3);

        assert_eq!(
            plan,
            [day("2026-09-22"), day("2026-09-21"), day("2026-09-20")]
        );
    }

    #[test]
    fn backfill_plan__unsealable_day_listed_an_hour_ago__leaves_it_out() {
        let now = oslo_ns("2026-09-24T10:00");
        let listed = [overflowed_listing("2026-09-22", now - HOUR)];

        let plan = backfill_plan(&listed, now, 2);

        assert!(!plan.contains(&day("2026-09-22")));
    }

    #[test]
    fn backfill_plan__unsealable_day_listed_25_hours_ago__lists_it_again() {
        let now = oslo_ns("2026-09-24T10:00");
        let listed = [overflowed_listing("2026-09-22", now - 25 * HOUR)];

        let plan = backfill_plan(&listed, now, 2);

        assert!(plan.contains(&day("2026-09-22")));
    }

    #[test]
    fn backfill_plan__day_listed_before_its_sealing_time__lists_it_again() {
        let listed = [ListedDay {
            listed_at_ns: oslo_ns("2026-09-23T01:00"),
            sealed: false,
            ..sealed_listing("2026-09-22")
        }];

        let plan = backfill_plan(&listed, now(), 1);

        assert_eq!(plan, [day("2026-09-22")]);
    }

    #[test]
    fn backfill_plan__every_day_sealed__is_empty() {
        let listed = sealed_listings("2026-09-16", "2026-09-22");

        let plan = backfill_plan(&listed, now(), 7);

        assert_eq!(plan, []);
    }

    fn client_error(source: TransportError) -> ClientError {
        ClientError::Transport {
            url: "https://api.example/v1/newsreader/message?messageId=7".to_owned(),
            source,
        }
    }

    #[test]
    fn failure__client_errors__is_permanent_only_when_the_error_is() {
        let cases = [
            (TransportError::Status(404), true),
            (TransportError::Status(410), true),
            (TransportError::Status(503), false),
            (TransportError::Timeout, false),
        ];
        for (source, expected) in cases {
            let description = source.to_string();
            assert_eq!(
                failure(&client_error(source)).permanent,
                expected,
                "error: {description}"
            );
        }
    }

    #[test]
    fn failure__client_error__has_the_error_text_as_detail() {
        let error = client_error(TransportError::Status(404));

        let failure = failure(&error);

        assert_eq!(
            failure.detail,
            "https://api.example/v1/newsreader/message?messageId=7: HTTP 404"
        );
    }

    #[test]
    fn format_ranges__ranges__joins_them_and_writes_one_day_as_a_date() {
        let ranges = [
            range("2026-09-16", "2026-09-17"),
            range("2026-09-20", "2026-09-20"),
        ];

        let text = format_ranges(&ranges);

        assert_eq!(text, "2026-09-16..2026-09-17, 2026-09-20");
    }

    #[test]
    fn format_ranges__no_range__returns_none() {
        let text = format_ranges(&[]);

        assert_eq!(text, "none");
    }

    #[test]
    fn day_range_days__last_before_first__yields_no_day() {
        let count = range("2026-09-23", "2026-09-22").days().count();

        assert_eq!(count, 0);
    }

    const INTERVAL: Duration = Duration::from_secs(60);

    /// Runs `repeat` with passes that return `results` in order and cancels
    /// it as the last pass starts. Returns when each pass started, in
    /// seconds of tokio's clock.
    async fn pass_starts(results: Vec<Result<usize, StoreError>>) -> Vec<u64> {
        let shutdown = CancellationToken::new();
        let start = Instant::now();
        let starts = std::sync::Mutex::new(Vec::new());
        let results = std::sync::Mutex::new(std::collections::VecDeque::from(results));
        repeat("test", INTERVAL, &shutdown, || {
            starts.lock().unwrap().push(start.elapsed().as_secs());
            let mut results = results.lock().unwrap();
            let result = results.pop_front().unwrap_or(Ok(0));
            if results.is_empty() {
                shutdown.cancel();
            }
            async move { result }
        })
        .await;
        starts.into_inner().unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn repeat__passes_find_no_work__waits_one_interval_between_them() {
        let starts = pass_starts(vec![Ok(0), Ok(0), Ok(0)]).await;

        assert_eq!(starts, [0, 60, 120]);
    }

    #[tokio::test(start_paused = true)]
    async fn repeat__pass_did_work__runs_the_next_at_once() {
        let starts = pass_starts(vec![Ok(20), Ok(3), Ok(0), Ok(0)]).await;

        assert_eq!(starts, [0, 0, 0, 60]);
    }

    #[tokio::test(start_paused = true)]
    async fn repeat__store_error__waits_one_interval_and_runs_again() {
        let error = StoreError::NoDisclosure { message_id: 7 };

        let starts = pass_starts(vec![Err(error), Ok(0)]).await;

        assert_eq!(starts, [0, 60]);
    }

    #[tokio::test(start_paused = true)]
    async fn repeat__cancelled_during_a_pass__returns_without_waiting_for_it() {
        let shutdown = CancellationToken::new();
        let pass_that_never_ends = std::future::pending::<Result<usize, StoreError>>;

        let stopped = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(
                repeat("test", INTERVAL, &shutdown, pass_that_never_ends),
                async { shutdown.cancel() },
            )
        })
        .await;

        assert!(stopped.is_ok());
    }

    #[test]
    fn due__unsealable_listing_just_over_a_day_old__returns_true() {
        let listing = overflowed_listing("2026-09-22", now() - RELIST_AFTER_NS - MINUTE);

        let due = due(Some(&listing), now());

        assert!(due);
    }
}
