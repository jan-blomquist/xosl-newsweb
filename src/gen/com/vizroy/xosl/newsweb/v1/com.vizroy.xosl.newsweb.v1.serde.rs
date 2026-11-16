// @generated
impl serde::Serialize for Attachment {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.id != 0 {
            len += 1;
        }
        if !self.name.is_empty() {
            len += 1;
        }
        if self.state != 0 {
            len += 1;
        }
        if !self.content_type.is_empty() {
            len += 1;
        }
        if self.size_bytes != 0 {
            len += 1;
        }
        if !self.sha256.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.v1.Attachment", len)?;
        if self.id != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("id", ToString::to_string(&self.id).as_str())?;
        }
        if !self.name.is_empty() {
            struct_ser.serialize_field("name", &self.name)?;
        }
        if self.state != 0 {
            let v = attachment::State::try_from(self.state)
                .map_err(|_| serde::ser::Error::custom(format!("Invalid variant {}", self.state)))?;
            struct_ser.serialize_field("state", &v)?;
        }
        if !self.content_type.is_empty() {
            struct_ser.serialize_field("contentType", &self.content_type)?;
        }
        if self.size_bytes != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("sizeBytes", ToString::to_string(&self.size_bytes).as_str())?;
        }
        if !self.sha256.is_empty() {
            struct_ser.serialize_field("sha256", &self.sha256)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Attachment {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "id",
            "name",
            "state",
            "content_type",
            "contentType",
            "size_bytes",
            "sizeBytes",
            "sha256",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Id,
            Name,
            State,
            ContentType,
            SizeBytes,
            Sha256,
            __SkipField__,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "id" => Ok(GeneratedField::Id),
                            "name" => Ok(GeneratedField::Name),
                            "state" => Ok(GeneratedField::State),
                            "contentType" | "content_type" => Ok(GeneratedField::ContentType),
                            "sizeBytes" | "size_bytes" => Ok(GeneratedField::SizeBytes),
                            "sha256" => Ok(GeneratedField::Sha256),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Attachment;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.v1.Attachment")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Attachment, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut id__ = None;
                let mut name__ = None;
                let mut state__ = None;
                let mut content_type__ = None;
                let mut size_bytes__ = None;
                let mut sha256__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Id => {
                            if id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("id"));
                            }
                            id__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::Name => {
                            if name__.is_some() {
                                return Err(serde::de::Error::duplicate_field("name"));
                            }
                            name__ = Some(map_.next_value()?);
                        }
                        GeneratedField::State => {
                            if state__.is_some() {
                                return Err(serde::de::Error::duplicate_field("state"));
                            }
                            state__ = Some(map_.next_value::<attachment::State>()? as i32);
                        }
                        GeneratedField::ContentType => {
                            if content_type__.is_some() {
                                return Err(serde::de::Error::duplicate_field("contentType"));
                            }
                            content_type__ = Some(map_.next_value()?);
                        }
                        GeneratedField::SizeBytes => {
                            if size_bytes__.is_some() {
                                return Err(serde::de::Error::duplicate_field("sizeBytes"));
                            }
                            size_bytes__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::Sha256 => {
                            if sha256__.is_some() {
                                return Err(serde::de::Error::duplicate_field("sha256"));
                            }
                            sha256__ = Some(map_.next_value()?);
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(Attachment {
                    id: id__.unwrap_or_default(),
                    name: name__.unwrap_or_default(),
                    state: state__.unwrap_or_default(),
                    content_type: content_type__.unwrap_or_default(),
                    size_bytes: size_bytes__.unwrap_or_default(),
                    sha256: sha256__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.v1.Attachment", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for attachment::State {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let variant = match self {
            Self::Unspecified => "STATE_UNSPECIFIED",
            Self::Pending => "STATE_PENDING",
            Self::Stored => "STATE_STORED",
            Self::Failed => "STATE_FAILED",
        };
        serializer.serialize_str(variant)
    }
}
impl<'de> serde::Deserialize<'de> for attachment::State {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "STATE_UNSPECIFIED",
            "STATE_PENDING",
            "STATE_STORED",
            "STATE_FAILED",
        ];

        struct GeneratedVisitor;

        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = attachment::State;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "expected one of: {:?}", &FIELDS)
            }

            fn visit_i64<E>(self, v: i64) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                i32::try_from(v)
                    .ok()
                    .and_then(|x| x.try_into().ok())
                    .ok_or_else(|| {
                        serde::de::Error::invalid_value(serde::de::Unexpected::Signed(v), &self)
                    })
            }

            fn visit_u64<E>(self, v: u64) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                i32::try_from(v)
                    .ok()
                    .and_then(|x| x.try_into().ok())
                    .ok_or_else(|| {
                        serde::de::Error::invalid_value(serde::de::Unexpected::Unsigned(v), &self)
                    })
            }

            fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "STATE_UNSPECIFIED" => Ok(attachment::State::Unspecified),
                    "STATE_PENDING" => Ok(attachment::State::Pending),
                    "STATE_STORED" => Ok(attachment::State::Stored),
                    "STATE_FAILED" => Ok(attachment::State::Failed),
                    _ => Err(serde::de::Error::unknown_variant(value, FIELDS)),
                }
            }
        }
        deserializer.deserialize_any(GeneratedVisitor)
    }
}
impl serde::Serialize for Disclosure {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.message_id != 0 {
            len += 1;
        }
        if self.news_id != 0 {
            len += 1;
        }
        if !self.client_announcement_id.is_empty() {
            len += 1;
        }
        if self.issuer.is_some() {
            len += 1;
        }
        if self.instrument.is_some() {
            len += 1;
        }
        if !self.markets.is_empty() {
            len += 1;
        }
        if self.published_at_ns != 0 {
            len += 1;
        }
        if self.received_at_ns != 0 {
            len += 1;
        }
        if !self.title.is_empty() {
            len += 1;
        }
        if !self.body.is_empty() {
            len += 1;
        }
        if self.read_in_full {
            len += 1;
        }
        if !self.category_ids.is_empty() {
            len += 1;
        }
        if !self.attachments.is_empty() {
            len += 1;
        }
        if self.oam_mandatory {
            len += 1;
        }
        if self.info_required {
            len += 1;
        }
        if self.test {
            len += 1;
        }
        if self.correction_for_message_id != 0 {
            len += 1;
        }
        if self.corrected_by_message_id != 0 {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.v1.Disclosure", len)?;
        if self.message_id != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("messageId", ToString::to_string(&self.message_id).as_str())?;
        }
        if self.news_id != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("newsId", ToString::to_string(&self.news_id).as_str())?;
        }
        if !self.client_announcement_id.is_empty() {
            struct_ser.serialize_field("clientAnnouncementId", &self.client_announcement_id)?;
        }
        if let Some(v) = self.issuer.as_ref() {
            struct_ser.serialize_field("issuer", v)?;
        }
        if let Some(v) = self.instrument.as_ref() {
            struct_ser.serialize_field("instrument", v)?;
        }
        if !self.markets.is_empty() {
            struct_ser.serialize_field("markets", &self.markets)?;
        }
        if self.published_at_ns != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("publishedAtNs", ToString::to_string(&self.published_at_ns).as_str())?;
        }
        if self.received_at_ns != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("receivedAtNs", ToString::to_string(&self.received_at_ns).as_str())?;
        }
        if !self.title.is_empty() {
            struct_ser.serialize_field("title", &self.title)?;
        }
        if !self.body.is_empty() {
            struct_ser.serialize_field("body", &self.body)?;
        }
        if self.read_in_full {
            struct_ser.serialize_field("readInFull", &self.read_in_full)?;
        }
        if !self.category_ids.is_empty() {
            struct_ser.serialize_field("categoryIds", &self.category_ids)?;
        }
        if !self.attachments.is_empty() {
            struct_ser.serialize_field("attachments", &self.attachments)?;
        }
        if self.oam_mandatory {
            struct_ser.serialize_field("oamMandatory", &self.oam_mandatory)?;
        }
        if self.info_required {
            struct_ser.serialize_field("infoRequired", &self.info_required)?;
        }
        if self.test {
            struct_ser.serialize_field("test", &self.test)?;
        }
        if self.correction_for_message_id != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("correctionForMessageId", ToString::to_string(&self.correction_for_message_id).as_str())?;
        }
        if self.corrected_by_message_id != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("correctedByMessageId", ToString::to_string(&self.corrected_by_message_id).as_str())?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Disclosure {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "message_id",
            "messageId",
            "news_id",
            "newsId",
            "client_announcement_id",
            "clientAnnouncementId",
            "issuer",
            "instrument",
            "markets",
            "published_at_ns",
            "publishedAtNs",
            "received_at_ns",
            "receivedAtNs",
            "title",
            "body",
            "read_in_full",
            "readInFull",
            "category_ids",
            "categoryIds",
            "attachments",
            "oam_mandatory",
            "oamMandatory",
            "info_required",
            "infoRequired",
            "test",
            "correction_for_message_id",
            "correctionForMessageId",
            "corrected_by_message_id",
            "correctedByMessageId",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            MessageId,
            NewsId,
            ClientAnnouncementId,
            Issuer,
            Instrument,
            Markets,
            PublishedAtNs,
            ReceivedAtNs,
            Title,
            Body,
            ReadInFull,
            CategoryIds,
            Attachments,
            OamMandatory,
            InfoRequired,
            Test,
            CorrectionForMessageId,
            CorrectedByMessageId,
            __SkipField__,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "messageId" | "message_id" => Ok(GeneratedField::MessageId),
                            "newsId" | "news_id" => Ok(GeneratedField::NewsId),
                            "clientAnnouncementId" | "client_announcement_id" => Ok(GeneratedField::ClientAnnouncementId),
                            "issuer" => Ok(GeneratedField::Issuer),
                            "instrument" => Ok(GeneratedField::Instrument),
                            "markets" => Ok(GeneratedField::Markets),
                            "publishedAtNs" | "published_at_ns" => Ok(GeneratedField::PublishedAtNs),
                            "receivedAtNs" | "received_at_ns" => Ok(GeneratedField::ReceivedAtNs),
                            "title" => Ok(GeneratedField::Title),
                            "body" => Ok(GeneratedField::Body),
                            "readInFull" | "read_in_full" => Ok(GeneratedField::ReadInFull),
                            "categoryIds" | "category_ids" => Ok(GeneratedField::CategoryIds),
                            "attachments" => Ok(GeneratedField::Attachments),
                            "oamMandatory" | "oam_mandatory" => Ok(GeneratedField::OamMandatory),
                            "infoRequired" | "info_required" => Ok(GeneratedField::InfoRequired),
                            "test" => Ok(GeneratedField::Test),
                            "correctionForMessageId" | "correction_for_message_id" => Ok(GeneratedField::CorrectionForMessageId),
                            "correctedByMessageId" | "corrected_by_message_id" => Ok(GeneratedField::CorrectedByMessageId),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Disclosure;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.v1.Disclosure")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Disclosure, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut message_id__ = None;
                let mut news_id__ = None;
                let mut client_announcement_id__ = None;
                let mut issuer__ = None;
                let mut instrument__ = None;
                let mut markets__ = None;
                let mut published_at_ns__ = None;
                let mut received_at_ns__ = None;
                let mut title__ = None;
                let mut body__ = None;
                let mut read_in_full__ = None;
                let mut category_ids__ = None;
                let mut attachments__ = None;
                let mut oam_mandatory__ = None;
                let mut info_required__ = None;
                let mut test__ = None;
                let mut correction_for_message_id__ = None;
                let mut corrected_by_message_id__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::MessageId => {
                            if message_id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("messageId"));
                            }
                            message_id__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::NewsId => {
                            if news_id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("newsId"));
                            }
                            news_id__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::ClientAnnouncementId => {
                            if client_announcement_id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("clientAnnouncementId"));
                            }
                            client_announcement_id__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Issuer => {
                            if issuer__.is_some() {
                                return Err(serde::de::Error::duplicate_field("issuer"));
                            }
                            issuer__ = map_.next_value()?;
                        }
                        GeneratedField::Instrument => {
                            if instrument__.is_some() {
                                return Err(serde::de::Error::duplicate_field("instrument"));
                            }
                            instrument__ = map_.next_value()?;
                        }
                        GeneratedField::Markets => {
                            if markets__.is_some() {
                                return Err(serde::de::Error::duplicate_field("markets"));
                            }
                            markets__ = Some(map_.next_value()?);
                        }
                        GeneratedField::PublishedAtNs => {
                            if published_at_ns__.is_some() {
                                return Err(serde::de::Error::duplicate_field("publishedAtNs"));
                            }
                            published_at_ns__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::ReceivedAtNs => {
                            if received_at_ns__.is_some() {
                                return Err(serde::de::Error::duplicate_field("receivedAtNs"));
                            }
                            received_at_ns__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::Title => {
                            if title__.is_some() {
                                return Err(serde::de::Error::duplicate_field("title"));
                            }
                            title__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Body => {
                            if body__.is_some() {
                                return Err(serde::de::Error::duplicate_field("body"));
                            }
                            body__ = Some(map_.next_value()?);
                        }
                        GeneratedField::ReadInFull => {
                            if read_in_full__.is_some() {
                                return Err(serde::de::Error::duplicate_field("readInFull"));
                            }
                            read_in_full__ = Some(map_.next_value()?);
                        }
                        GeneratedField::CategoryIds => {
                            if category_ids__.is_some() {
                                return Err(serde::de::Error::duplicate_field("categoryIds"));
                            }
                            category_ids__ = 
                                Some(map_.next_value::<Vec<::pbjson::private::NumberDeserialize<_>>>()?
                                    .into_iter().map(|x| x.0).collect())
                            ;
                        }
                        GeneratedField::Attachments => {
                            if attachments__.is_some() {
                                return Err(serde::de::Error::duplicate_field("attachments"));
                            }
                            attachments__ = Some(map_.next_value()?);
                        }
                        GeneratedField::OamMandatory => {
                            if oam_mandatory__.is_some() {
                                return Err(serde::de::Error::duplicate_field("oamMandatory"));
                            }
                            oam_mandatory__ = Some(map_.next_value()?);
                        }
                        GeneratedField::InfoRequired => {
                            if info_required__.is_some() {
                                return Err(serde::de::Error::duplicate_field("infoRequired"));
                            }
                            info_required__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Test => {
                            if test__.is_some() {
                                return Err(serde::de::Error::duplicate_field("test"));
                            }
                            test__ = Some(map_.next_value()?);
                        }
                        GeneratedField::CorrectionForMessageId => {
                            if correction_for_message_id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("correctionForMessageId"));
                            }
                            correction_for_message_id__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::CorrectedByMessageId => {
                            if corrected_by_message_id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("correctedByMessageId"));
                            }
                            corrected_by_message_id__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(Disclosure {
                    message_id: message_id__.unwrap_or_default(),
                    news_id: news_id__.unwrap_or_default(),
                    client_announcement_id: client_announcement_id__.unwrap_or_default(),
                    issuer: issuer__,
                    instrument: instrument__,
                    markets: markets__.unwrap_or_default(),
                    published_at_ns: published_at_ns__.unwrap_or_default(),
                    received_at_ns: received_at_ns__.unwrap_or_default(),
                    title: title__.unwrap_or_default(),
                    body: body__.unwrap_or_default(),
                    read_in_full: read_in_full__.unwrap_or_default(),
                    category_ids: category_ids__.unwrap_or_default(),
                    attachments: attachments__.unwrap_or_default(),
                    oam_mandatory: oam_mandatory__.unwrap_or_default(),
                    info_required: info_required__.unwrap_or_default(),
                    test: test__.unwrap_or_default(),
                    correction_for_message_id: correction_for_message_id__.unwrap_or_default(),
                    corrected_by_message_id: corrected_by_message_id__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.v1.Disclosure", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for DisclosureFilter {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.published_from_ns != 0 {
            len += 1;
        }
        if self.published_before_ns != 0 {
            len += 1;
        }
        if !self.issuer_signs.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.v1.DisclosureFilter", len)?;
        if self.published_from_ns != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("publishedFromNs", ToString::to_string(&self.published_from_ns).as_str())?;
        }
        if self.published_before_ns != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("publishedBeforeNs", ToString::to_string(&self.published_before_ns).as_str())?;
        }
        if !self.issuer_signs.is_empty() {
            struct_ser.serialize_field("issuerSigns", &self.issuer_signs)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for DisclosureFilter {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "published_from_ns",
            "publishedFromNs",
            "published_before_ns",
            "publishedBeforeNs",
            "issuer_signs",
            "issuerSigns",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            PublishedFromNs,
            PublishedBeforeNs,
            IssuerSigns,
            __SkipField__,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "publishedFromNs" | "published_from_ns" => Ok(GeneratedField::PublishedFromNs),
                            "publishedBeforeNs" | "published_before_ns" => Ok(GeneratedField::PublishedBeforeNs),
                            "issuerSigns" | "issuer_signs" => Ok(GeneratedField::IssuerSigns),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = DisclosureFilter;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.v1.DisclosureFilter")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<DisclosureFilter, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut published_from_ns__ = None;
                let mut published_before_ns__ = None;
                let mut issuer_signs__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::PublishedFromNs => {
                            if published_from_ns__.is_some() {
                                return Err(serde::de::Error::duplicate_field("publishedFromNs"));
                            }
                            published_from_ns__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::PublishedBeforeNs => {
                            if published_before_ns__.is_some() {
                                return Err(serde::de::Error::duplicate_field("publishedBeforeNs"));
                            }
                            published_before_ns__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::IssuerSigns => {
                            if issuer_signs__.is_some() {
                                return Err(serde::de::Error::duplicate_field("issuerSigns"));
                            }
                            issuer_signs__ = Some(map_.next_value()?);
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(DisclosureFilter {
                    published_from_ns: published_from_ns__.unwrap_or_default(),
                    published_before_ns: published_before_ns__.unwrap_or_default(),
                    issuer_signs: issuer_signs__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.v1.DisclosureFilter", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for Event {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.seq != 0 {
            len += 1;
        }
        if self.kind != 0 {
            len += 1;
        }
        if self.at_ns != 0 {
            len += 1;
        }
        if self.disclosure.is_some() {
            len += 1;
        }
        if self.attachment_id != 0 {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.v1.Event", len)?;
        if self.seq != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("seq", ToString::to_string(&self.seq).as_str())?;
        }
        if self.kind != 0 {
            let v = event::Kind::try_from(self.kind)
                .map_err(|_| serde::ser::Error::custom(format!("Invalid variant {}", self.kind)))?;
            struct_ser.serialize_field("kind", &v)?;
        }
        if self.at_ns != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("atNs", ToString::to_string(&self.at_ns).as_str())?;
        }
        if let Some(v) = self.disclosure.as_ref() {
            struct_ser.serialize_field("disclosure", v)?;
        }
        if self.attachment_id != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("attachmentId", ToString::to_string(&self.attachment_id).as_str())?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Event {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "seq",
            "kind",
            "at_ns",
            "atNs",
            "disclosure",
            "attachment_id",
            "attachmentId",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Seq,
            Kind,
            AtNs,
            Disclosure,
            AttachmentId,
            __SkipField__,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "seq" => Ok(GeneratedField::Seq),
                            "kind" => Ok(GeneratedField::Kind),
                            "atNs" | "at_ns" => Ok(GeneratedField::AtNs),
                            "disclosure" => Ok(GeneratedField::Disclosure),
                            "attachmentId" | "attachment_id" => Ok(GeneratedField::AttachmentId),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Event;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.v1.Event")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Event, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut seq__ = None;
                let mut kind__ = None;
                let mut at_ns__ = None;
                let mut disclosure__ = None;
                let mut attachment_id__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Seq => {
                            if seq__.is_some() {
                                return Err(serde::de::Error::duplicate_field("seq"));
                            }
                            seq__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::Kind => {
                            if kind__.is_some() {
                                return Err(serde::de::Error::duplicate_field("kind"));
                            }
                            kind__ = Some(map_.next_value::<event::Kind>()? as i32);
                        }
                        GeneratedField::AtNs => {
                            if at_ns__.is_some() {
                                return Err(serde::de::Error::duplicate_field("atNs"));
                            }
                            at_ns__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::Disclosure => {
                            if disclosure__.is_some() {
                                return Err(serde::de::Error::duplicate_field("disclosure"));
                            }
                            disclosure__ = map_.next_value()?;
                        }
                        GeneratedField::AttachmentId => {
                            if attachment_id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("attachmentId"));
                            }
                            attachment_id__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(Event {
                    seq: seq__.unwrap_or_default(),
                    kind: kind__.unwrap_or_default(),
                    at_ns: at_ns__.unwrap_or_default(),
                    disclosure: disclosure__,
                    attachment_id: attachment_id__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.v1.Event", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for event::Kind {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let variant = match self {
            Self::Unspecified => "KIND_UNSPECIFIED",
            Self::Discovered => "KIND_DISCOVERED",
            Self::Revised => "KIND_REVISED",
            Self::Corrected => "KIND_CORRECTED",
            Self::AttachmentStored => "KIND_ATTACHMENT_STORED",
            Self::AttachmentFailed => "KIND_ATTACHMENT_FAILED",
        };
        serializer.serialize_str(variant)
    }
}
impl<'de> serde::Deserialize<'de> for event::Kind {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "KIND_UNSPECIFIED",
            "KIND_DISCOVERED",
            "KIND_REVISED",
            "KIND_CORRECTED",
            "KIND_ATTACHMENT_STORED",
            "KIND_ATTACHMENT_FAILED",
        ];

        struct GeneratedVisitor;

        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = event::Kind;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "expected one of: {:?}", &FIELDS)
            }

            fn visit_i64<E>(self, v: i64) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                i32::try_from(v)
                    .ok()
                    .and_then(|x| x.try_into().ok())
                    .ok_or_else(|| {
                        serde::de::Error::invalid_value(serde::de::Unexpected::Signed(v), &self)
                    })
            }

            fn visit_u64<E>(self, v: u64) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                i32::try_from(v)
                    .ok()
                    .and_then(|x| x.try_into().ok())
                    .ok_or_else(|| {
                        serde::de::Error::invalid_value(serde::de::Unexpected::Unsigned(v), &self)
                    })
            }

            fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "KIND_UNSPECIFIED" => Ok(event::Kind::Unspecified),
                    "KIND_DISCOVERED" => Ok(event::Kind::Discovered),
                    "KIND_REVISED" => Ok(event::Kind::Revised),
                    "KIND_CORRECTED" => Ok(event::Kind::Corrected),
                    "KIND_ATTACHMENT_STORED" => Ok(event::Kind::AttachmentStored),
                    "KIND_ATTACHMENT_FAILED" => Ok(event::Kind::AttachmentFailed),
                    _ => Err(serde::de::Error::unknown_variant(value, FIELDS)),
                }
            }
        }
        deserializer.deserialize_any(GeneratedVisitor)
    }
}
impl serde::Serialize for GetDisclosureRequest {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.message_id != 0 {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.v1.GetDisclosureRequest", len)?;
        if self.message_id != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("messageId", ToString::to_string(&self.message_id).as_str())?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for GetDisclosureRequest {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "message_id",
            "messageId",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            MessageId,
            __SkipField__,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "messageId" | "message_id" => Ok(GeneratedField::MessageId),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = GetDisclosureRequest;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.v1.GetDisclosureRequest")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<GetDisclosureRequest, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut message_id__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::MessageId => {
                            if message_id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("messageId"));
                            }
                            message_id__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(GetDisclosureRequest {
                    message_id: message_id__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.v1.GetDisclosureRequest", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for GetDisclosureResponse {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.disclosure.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.v1.GetDisclosureResponse", len)?;
        if let Some(v) = self.disclosure.as_ref() {
            struct_ser.serialize_field("disclosure", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for GetDisclosureResponse {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "disclosure",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Disclosure,
            __SkipField__,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "disclosure" => Ok(GeneratedField::Disclosure),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = GetDisclosureResponse;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.v1.GetDisclosureResponse")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<GetDisclosureResponse, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut disclosure__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Disclosure => {
                            if disclosure__.is_some() {
                                return Err(serde::de::Error::duplicate_field("disclosure"));
                            }
                            disclosure__ = map_.next_value()?;
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(GetDisclosureResponse {
                    disclosure: disclosure__,
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.v1.GetDisclosureResponse", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for Instrument {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.id != 0 {
            len += 1;
        }
        if !self.name.is_empty() {
            len += 1;
        }
        if !self.full_name.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.v1.Instrument", len)?;
        if self.id != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("id", ToString::to_string(&self.id).as_str())?;
        }
        if !self.name.is_empty() {
            struct_ser.serialize_field("name", &self.name)?;
        }
        if !self.full_name.is_empty() {
            struct_ser.serialize_field("fullName", &self.full_name)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Instrument {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "id",
            "name",
            "full_name",
            "fullName",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Id,
            Name,
            FullName,
            __SkipField__,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "id" => Ok(GeneratedField::Id),
                            "name" => Ok(GeneratedField::Name),
                            "fullName" | "full_name" => Ok(GeneratedField::FullName),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Instrument;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.v1.Instrument")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Instrument, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut id__ = None;
                let mut name__ = None;
                let mut full_name__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Id => {
                            if id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("id"));
                            }
                            id__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::Name => {
                            if name__.is_some() {
                                return Err(serde::de::Error::duplicate_field("name"));
                            }
                            name__ = Some(map_.next_value()?);
                        }
                        GeneratedField::FullName => {
                            if full_name__.is_some() {
                                return Err(serde::de::Error::duplicate_field("fullName"));
                            }
                            full_name__ = Some(map_.next_value()?);
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(Instrument {
                    id: id__.unwrap_or_default(),
                    name: name__.unwrap_or_default(),
                    full_name: full_name__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.v1.Instrument", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for Issuer {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.id != 0 {
            len += 1;
        }
        if !self.sign.is_empty() {
            len += 1;
        }
        if !self.name.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.v1.Issuer", len)?;
        if self.id != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("id", ToString::to_string(&self.id).as_str())?;
        }
        if !self.sign.is_empty() {
            struct_ser.serialize_field("sign", &self.sign)?;
        }
        if !self.name.is_empty() {
            struct_ser.serialize_field("name", &self.name)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Issuer {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "id",
            "sign",
            "name",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Id,
            Sign,
            Name,
            __SkipField__,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "id" => Ok(GeneratedField::Id),
                            "sign" => Ok(GeneratedField::Sign),
                            "name" => Ok(GeneratedField::Name),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Issuer;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.v1.Issuer")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Issuer, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut id__ = None;
                let mut sign__ = None;
                let mut name__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Id => {
                            if id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("id"));
                            }
                            id__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::Sign => {
                            if sign__.is_some() {
                                return Err(serde::de::Error::duplicate_field("sign"));
                            }
                            sign__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Name => {
                            if name__.is_some() {
                                return Err(serde::de::Error::duplicate_field("name"));
                            }
                            name__ = Some(map_.next_value()?);
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(Issuer {
                    id: id__.unwrap_or_default(),
                    sign: sign__.unwrap_or_default(),
                    name: name__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.v1.Issuer", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for ListDisclosuresRequest {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.filter.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.v1.ListDisclosuresRequest", len)?;
        if let Some(v) = self.filter.as_ref() {
            struct_ser.serialize_field("filter", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for ListDisclosuresRequest {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "filter",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Filter,
            __SkipField__,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "filter" => Ok(GeneratedField::Filter),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = ListDisclosuresRequest;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.v1.ListDisclosuresRequest")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<ListDisclosuresRequest, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut filter__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Filter => {
                            if filter__.is_some() {
                                return Err(serde::de::Error::duplicate_field("filter"));
                            }
                            filter__ = map_.next_value()?;
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(ListDisclosuresRequest {
                    filter: filter__,
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.v1.ListDisclosuresRequest", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for ListDisclosuresResponse {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.disclosure.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.v1.ListDisclosuresResponse", len)?;
        if let Some(v) = self.disclosure.as_ref() {
            struct_ser.serialize_field("disclosure", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for ListDisclosuresResponse {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "disclosure",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Disclosure,
            __SkipField__,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "disclosure" => Ok(GeneratedField::Disclosure),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = ListDisclosuresResponse;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.v1.ListDisclosuresResponse")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<ListDisclosuresResponse, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut disclosure__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Disclosure => {
                            if disclosure__.is_some() {
                                return Err(serde::de::Error::duplicate_field("disclosure"));
                            }
                            disclosure__ = map_.next_value()?;
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(ListDisclosuresResponse {
                    disclosure: disclosure__,
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.v1.ListDisclosuresResponse", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for ReadAttachmentRequest {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.message_id != 0 {
            len += 1;
        }
        if self.attachment_id != 0 {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.v1.ReadAttachmentRequest", len)?;
        if self.message_id != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("messageId", ToString::to_string(&self.message_id).as_str())?;
        }
        if self.attachment_id != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("attachmentId", ToString::to_string(&self.attachment_id).as_str())?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for ReadAttachmentRequest {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "message_id",
            "messageId",
            "attachment_id",
            "attachmentId",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            MessageId,
            AttachmentId,
            __SkipField__,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "messageId" | "message_id" => Ok(GeneratedField::MessageId),
                            "attachmentId" | "attachment_id" => Ok(GeneratedField::AttachmentId),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = ReadAttachmentRequest;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.v1.ReadAttachmentRequest")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<ReadAttachmentRequest, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut message_id__ = None;
                let mut attachment_id__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::MessageId => {
                            if message_id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("messageId"));
                            }
                            message_id__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::AttachmentId => {
                            if attachment_id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("attachmentId"));
                            }
                            attachment_id__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(ReadAttachmentRequest {
                    message_id: message_id__.unwrap_or_default(),
                    attachment_id: attachment_id__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.v1.ReadAttachmentRequest", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for ReadAttachmentResponse {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if !self.chunk.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.v1.ReadAttachmentResponse", len)?;
        if !self.chunk.is_empty() {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("chunk", pbjson::private::base64::encode(&self.chunk).as_str())?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for ReadAttachmentResponse {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "chunk",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Chunk,
            __SkipField__,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "chunk" => Ok(GeneratedField::Chunk),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = ReadAttachmentResponse;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.v1.ReadAttachmentResponse")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<ReadAttachmentResponse, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut chunk__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Chunk => {
                            if chunk__.is_some() {
                                return Err(serde::de::Error::duplicate_field("chunk"));
                            }
                            chunk__ = 
                                Some(map_.next_value::<::pbjson::private::BytesDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(ReadAttachmentResponse {
                    chunk: chunk__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.v1.ReadAttachmentResponse", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for WatchEventsRequest {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.since_seq != 0 {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.v1.WatchEventsRequest", len)?;
        if self.since_seq != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("sinceSeq", ToString::to_string(&self.since_seq).as_str())?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for WatchEventsRequest {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "since_seq",
            "sinceSeq",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            SinceSeq,
            __SkipField__,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "sinceSeq" | "since_seq" => Ok(GeneratedField::SinceSeq),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = WatchEventsRequest;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.v1.WatchEventsRequest")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<WatchEventsRequest, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut since_seq__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::SinceSeq => {
                            if since_seq__.is_some() {
                                return Err(serde::de::Error::duplicate_field("sinceSeq"));
                            }
                            since_seq__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(WatchEventsRequest {
                    since_seq: since_seq__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.v1.WatchEventsRequest", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for WatchEventsResponse {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.event.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.v1.WatchEventsResponse", len)?;
        if let Some(v) = self.event.as_ref() {
            struct_ser.serialize_field("event", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for WatchEventsResponse {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "event",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Event,
            __SkipField__,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "event" => Ok(GeneratedField::Event),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = WatchEventsResponse;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.v1.WatchEventsResponse")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<WatchEventsResponse, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut event__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Event => {
                            if event__.is_some() {
                                return Err(serde::de::Error::duplicate_field("event"));
                            }
                            event__ = map_.next_value()?;
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(WatchEventsResponse {
                    event: event__,
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.v1.WatchEventsResponse", FIELDS, GeneratedVisitor)
    }
}
