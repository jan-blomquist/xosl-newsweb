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
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.raw.v1.Attachment", len)?;
        if self.id != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("id", ToString::to_string(&self.id).as_str())?;
        }
        if !self.name.is_empty() {
            struct_ser.serialize_field("name", &self.name)?;
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
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Id,
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
            type Value = Attachment;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.raw.v1.Attachment")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Attachment, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut id__ = None;
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
                Ok(Attachment {
                    id: id__.unwrap_or_default(),
                    name: name__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.raw.v1.Attachment", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for Category {
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
        if !self.category_no.is_empty() {
            len += 1;
        }
        if !self.category_en.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.raw.v1.Category", len)?;
        if self.id != 0 {
            struct_ser.serialize_field("id", &self.id)?;
        }
        if !self.category_no.is_empty() {
            struct_ser.serialize_field("categoryNo", &self.category_no)?;
        }
        if !self.category_en.is_empty() {
            struct_ser.serialize_field("categoryEn", &self.category_en)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Category {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "id",
            "category_no",
            "categoryNo",
            "category_en",
            "categoryEn",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Id,
            CategoryNo,
            CategoryEn,
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
                            "categoryNo" | "category_no" => Ok(GeneratedField::CategoryNo),
                            "categoryEn" | "category_en" => Ok(GeneratedField::CategoryEn),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Category;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.raw.v1.Category")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Category, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut id__ = None;
                let mut category_no__ = None;
                let mut category_en__ = None;
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
                        GeneratedField::CategoryNo => {
                            if category_no__.is_some() {
                                return Err(serde::de::Error::duplicate_field("categoryNo"));
                            }
                            category_no__ = Some(map_.next_value()?);
                        }
                        GeneratedField::CategoryEn => {
                            if category_en__.is_some() {
                                return Err(serde::de::Error::duplicate_field("categoryEn"));
                            }
                            category_en__ = Some(map_.next_value()?);
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(Category {
                    id: id__.unwrap_or_default(),
                    category_no: category_no__.unwrap_or_default(),
                    category_en: category_en__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.raw.v1.Category", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for Header {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.http_code.is_some() {
            len += 1;
        }
        if !self.result_text.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.raw.v1.Header", len)?;
        if let Some(v) = self.http_code.as_ref() {
            struct_ser.serialize_field("http.code", v)?;
        }
        if !self.result_text.is_empty() {
            struct_ser.serialize_field("result.text", &self.result_text)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Header {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "http_code",
            "http.code",
            "result_text",
            "result.text",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            HttpCode,
            ResultText,
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
                            "http.code" | "http_code" => Ok(GeneratedField::HttpCode),
                            "result.text" | "result_text" => Ok(GeneratedField::ResultText),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Header;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.raw.v1.Header")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Header, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut http_code__ = None;
                let mut result_text__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::HttpCode => {
                            if http_code__.is_some() {
                                return Err(serde::de::Error::duplicate_field("http.code"));
                            }
                            http_code__ = 
                                map_.next_value::<::std::option::Option<::pbjson::private::NumberDeserialize<_>>>()?.map(|x| x.0)
                            ;
                        }
                        GeneratedField::ResultText => {
                            if result_text__.is_some() {
                                return Err(serde::de::Error::duplicate_field("result.text"));
                            }
                            result_text__ = Some(map_.next_value()?);
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(Header {
                    http_code: http_code__,
                    result_text: result_text__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.raw.v1.Header", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for ListData {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if !self.messages.is_empty() {
            len += 1;
        }
        if self.overflow {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.raw.v1.ListData", len)?;
        if !self.messages.is_empty() {
            struct_ser.serialize_field("messages", &self.messages)?;
        }
        if self.overflow {
            struct_ser.serialize_field("overflow", &self.overflow)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for ListData {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "messages",
            "overflow",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Messages,
            Overflow,
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
                            "messages" => Ok(GeneratedField::Messages),
                            "overflow" => Ok(GeneratedField::Overflow),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = ListData;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.raw.v1.ListData")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<ListData, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut messages__ = None;
                let mut overflow__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Messages => {
                            if messages__.is_some() {
                                return Err(serde::de::Error::duplicate_field("messages"));
                            }
                            messages__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Overflow => {
                            if overflow__.is_some() {
                                return Err(serde::de::Error::duplicate_field("overflow"));
                            }
                            overflow__ = Some(map_.next_value()?);
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(ListData {
                    messages: messages__.unwrap_or_default(),
                    overflow: overflow__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.raw.v1.ListData", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for ListResponse {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.header.is_some() {
            len += 1;
        }
        if self.data.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.raw.v1.ListResponse", len)?;
        if let Some(v) = self.header.as_ref() {
            struct_ser.serialize_field("header", v)?;
        }
        if let Some(v) = self.data.as_ref() {
            struct_ser.serialize_field("data", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for ListResponse {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "header",
            "data",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Header,
            Data,
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
                            "header" => Ok(GeneratedField::Header),
                            "data" => Ok(GeneratedField::Data),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = ListResponse;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.raw.v1.ListResponse")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<ListResponse, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut header__ = None;
                let mut data__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Header => {
                            if header__.is_some() {
                                return Err(serde::de::Error::duplicate_field("header"));
                            }
                            header__ = map_.next_value()?;
                        }
                        GeneratedField::Data => {
                            if data__.is_some() {
                                return Err(serde::de::Error::duplicate_field("data"));
                            }
                            data__ = map_.next_value()?;
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(ListResponse {
                    header: header__,
                    data: data__,
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.raw.v1.ListResponse", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for Message {
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
        if self.issuer_id != 0 {
            len += 1;
        }
        if !self.issuer_name.is_empty() {
            len += 1;
        }
        if !self.issuer_sign.is_empty() {
            len += 1;
        }
        if self.instr_id != 0 {
            len += 1;
        }
        if !self.instrument_name.is_empty() {
            len += 1;
        }
        if !self.instrument_full_name.is_empty() {
            len += 1;
        }
        if !self.markets.is_empty() {
            len += 1;
        }
        if !self.published_time.is_empty() {
            len += 1;
        }
        if !self.title.is_empty() {
            len += 1;
        }
        if !self.body.is_empty() {
            len += 1;
        }
        if !self.category.is_empty() {
            len += 1;
        }
        if !self.attachments.is_empty() {
            len += 1;
        }
        if self.numb_attachments != 0 {
            len += 1;
        }
        if self.oam_mandatory != 0 {
            len += 1;
        }
        if self.info_required != 0 {
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
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.raw.v1.Message", len)?;
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
        if self.issuer_id != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("issuerId", ToString::to_string(&self.issuer_id).as_str())?;
        }
        if !self.issuer_name.is_empty() {
            struct_ser.serialize_field("issuerName", &self.issuer_name)?;
        }
        if !self.issuer_sign.is_empty() {
            struct_ser.serialize_field("issuerSign", &self.issuer_sign)?;
        }
        if self.instr_id != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("instrId", ToString::to_string(&self.instr_id).as_str())?;
        }
        if !self.instrument_name.is_empty() {
            struct_ser.serialize_field("instrumentName", &self.instrument_name)?;
        }
        if !self.instrument_full_name.is_empty() {
            struct_ser.serialize_field("instrumentFullName", &self.instrument_full_name)?;
        }
        if !self.markets.is_empty() {
            struct_ser.serialize_field("markets", &self.markets)?;
        }
        if !self.published_time.is_empty() {
            struct_ser.serialize_field("publishedTime", &self.published_time)?;
        }
        if !self.title.is_empty() {
            struct_ser.serialize_field("title", &self.title)?;
        }
        if !self.body.is_empty() {
            struct_ser.serialize_field("body", &self.body)?;
        }
        if !self.category.is_empty() {
            struct_ser.serialize_field("category", &self.category)?;
        }
        if !self.attachments.is_empty() {
            struct_ser.serialize_field("attachments", &self.attachments)?;
        }
        if self.numb_attachments != 0 {
            struct_ser.serialize_field("numbAttachments", &self.numb_attachments)?;
        }
        if self.oam_mandatory != 0 {
            struct_ser.serialize_field("oamMandatory", &self.oam_mandatory)?;
        }
        if self.info_required != 0 {
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
impl<'de> serde::Deserialize<'de> for Message {
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
            "issuer_id",
            "issuerId",
            "issuer_name",
            "issuerName",
            "issuer_sign",
            "issuerSign",
            "instr_id",
            "instrId",
            "instrument_name",
            "instrumentName",
            "instrument_full_name",
            "instrumentFullName",
            "markets",
            "published_time",
            "publishedTime",
            "title",
            "body",
            "category",
            "attachments",
            "numb_attachments",
            "numbAttachments",
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
            IssuerId,
            IssuerName,
            IssuerSign,
            InstrId,
            InstrumentName,
            InstrumentFullName,
            Markets,
            PublishedTime,
            Title,
            Body,
            Category,
            Attachments,
            NumbAttachments,
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
                            "issuerId" | "issuer_id" => Ok(GeneratedField::IssuerId),
                            "issuerName" | "issuer_name" => Ok(GeneratedField::IssuerName),
                            "issuerSign" | "issuer_sign" => Ok(GeneratedField::IssuerSign),
                            "instrId" | "instr_id" => Ok(GeneratedField::InstrId),
                            "instrumentName" | "instrument_name" => Ok(GeneratedField::InstrumentName),
                            "instrumentFullName" | "instrument_full_name" => Ok(GeneratedField::InstrumentFullName),
                            "markets" => Ok(GeneratedField::Markets),
                            "publishedTime" | "published_time" => Ok(GeneratedField::PublishedTime),
                            "title" => Ok(GeneratedField::Title),
                            "body" => Ok(GeneratedField::Body),
                            "category" => Ok(GeneratedField::Category),
                            "attachments" => Ok(GeneratedField::Attachments),
                            "numbAttachments" | "numb_attachments" => Ok(GeneratedField::NumbAttachments),
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
            type Value = Message;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.raw.v1.Message")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Message, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut message_id__ = None;
                let mut news_id__ = None;
                let mut client_announcement_id__ = None;
                let mut issuer_id__ = None;
                let mut issuer_name__ = None;
                let mut issuer_sign__ = None;
                let mut instr_id__ = None;
                let mut instrument_name__ = None;
                let mut instrument_full_name__ = None;
                let mut markets__ = None;
                let mut published_time__ = None;
                let mut title__ = None;
                let mut body__ = None;
                let mut category__ = None;
                let mut attachments__ = None;
                let mut numb_attachments__ = None;
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
                        GeneratedField::IssuerId => {
                            if issuer_id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("issuerId"));
                            }
                            issuer_id__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::IssuerName => {
                            if issuer_name__.is_some() {
                                return Err(serde::de::Error::duplicate_field("issuerName"));
                            }
                            issuer_name__ = Some(map_.next_value()?);
                        }
                        GeneratedField::IssuerSign => {
                            if issuer_sign__.is_some() {
                                return Err(serde::de::Error::duplicate_field("issuerSign"));
                            }
                            issuer_sign__ = Some(map_.next_value()?);
                        }
                        GeneratedField::InstrId => {
                            if instr_id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("instrId"));
                            }
                            instr_id__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::InstrumentName => {
                            if instrument_name__.is_some() {
                                return Err(serde::de::Error::duplicate_field("instrumentName"));
                            }
                            instrument_name__ = Some(map_.next_value()?);
                        }
                        GeneratedField::InstrumentFullName => {
                            if instrument_full_name__.is_some() {
                                return Err(serde::de::Error::duplicate_field("instrumentFullName"));
                            }
                            instrument_full_name__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Markets => {
                            if markets__.is_some() {
                                return Err(serde::de::Error::duplicate_field("markets"));
                            }
                            markets__ = Some(map_.next_value()?);
                        }
                        GeneratedField::PublishedTime => {
                            if published_time__.is_some() {
                                return Err(serde::de::Error::duplicate_field("publishedTime"));
                            }
                            published_time__ = Some(map_.next_value()?);
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
                        GeneratedField::Category => {
                            if category__.is_some() {
                                return Err(serde::de::Error::duplicate_field("category"));
                            }
                            category__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Attachments => {
                            if attachments__.is_some() {
                                return Err(serde::de::Error::duplicate_field("attachments"));
                            }
                            attachments__ = Some(map_.next_value()?);
                        }
                        GeneratedField::NumbAttachments => {
                            if numb_attachments__.is_some() {
                                return Err(serde::de::Error::duplicate_field("numbAttachments"));
                            }
                            numb_attachments__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::OamMandatory => {
                            if oam_mandatory__.is_some() {
                                return Err(serde::de::Error::duplicate_field("oamMandatory"));
                            }
                            oam_mandatory__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::InfoRequired => {
                            if info_required__.is_some() {
                                return Err(serde::de::Error::duplicate_field("infoRequired"));
                            }
                            info_required__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
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
                Ok(Message {
                    message_id: message_id__.unwrap_or_default(),
                    news_id: news_id__.unwrap_or_default(),
                    client_announcement_id: client_announcement_id__.unwrap_or_default(),
                    issuer_id: issuer_id__.unwrap_or_default(),
                    issuer_name: issuer_name__.unwrap_or_default(),
                    issuer_sign: issuer_sign__.unwrap_or_default(),
                    instr_id: instr_id__.unwrap_or_default(),
                    instrument_name: instrument_name__.unwrap_or_default(),
                    instrument_full_name: instrument_full_name__.unwrap_or_default(),
                    markets: markets__.unwrap_or_default(),
                    published_time: published_time__.unwrap_or_default(),
                    title: title__.unwrap_or_default(),
                    body: body__.unwrap_or_default(),
                    category: category__.unwrap_or_default(),
                    attachments: attachments__.unwrap_or_default(),
                    numb_attachments: numb_attachments__.unwrap_or_default(),
                    oam_mandatory: oam_mandatory__.unwrap_or_default(),
                    info_required: info_required__.unwrap_or_default(),
                    test: test__.unwrap_or_default(),
                    correction_for_message_id: correction_for_message_id__.unwrap_or_default(),
                    corrected_by_message_id: corrected_by_message_id__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.raw.v1.Message", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for MessageData {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.message.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.raw.v1.MessageData", len)?;
        if let Some(v) = self.message.as_ref() {
            struct_ser.serialize_field("message", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for MessageData {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "message",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Message,
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
                            "message" => Ok(GeneratedField::Message),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = MessageData;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.raw.v1.MessageData")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<MessageData, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut message__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Message => {
                            if message__.is_some() {
                                return Err(serde::de::Error::duplicate_field("message"));
                            }
                            message__ = map_.next_value()?;
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(MessageData {
                    message: message__,
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.raw.v1.MessageData", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for MessageResponse {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.header.is_some() {
            len += 1;
        }
        if self.data.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.raw.v1.MessageResponse", len)?;
        if let Some(v) = self.header.as_ref() {
            struct_ser.serialize_field("header", v)?;
        }
        if let Some(v) = self.data.as_ref() {
            struct_ser.serialize_field("data", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for MessageResponse {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "header",
            "data",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Header,
            Data,
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
                            "header" => Ok(GeneratedField::Header),
                            "data" => Ok(GeneratedField::Data),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = MessageResponse;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.raw.v1.MessageResponse")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<MessageResponse, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut header__ = None;
                let mut data__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Header => {
                            if header__.is_some() {
                                return Err(serde::de::Error::duplicate_field("header"));
                            }
                            header__ = map_.next_value()?;
                        }
                        GeneratedField::Data => {
                            if data__.is_some() {
                                return Err(serde::de::Error::duplicate_field("data"));
                            }
                            data__ = map_.next_value()?;
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(MessageResponse {
                    header: header__,
                    data: data__,
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.raw.v1.MessageResponse", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for Urls {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if !self.version.is_empty() {
            len += 1;
        }
        if !self.build_time.is_empty() {
            len += 1;
        }
        if !self.api_large.is_empty() {
            len += 1;
        }
        if !self.public_url.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("com.vizroy.xosl.newsweb.raw.v1.Urls", len)?;
        if !self.version.is_empty() {
            struct_ser.serialize_field("version", &self.version)?;
        }
        if !self.build_time.is_empty() {
            struct_ser.serialize_field("buildTime", &self.build_time)?;
        }
        if !self.api_large.is_empty() {
            struct_ser.serialize_field("apiLarge", &self.api_large)?;
        }
        if !self.public_url.is_empty() {
            struct_ser.serialize_field("publicUrl", &self.public_url)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Urls {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "version",
            "build_time",
            "buildTime",
            "api_large",
            "apiLarge",
            "public_url",
            "publicUrl",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Version,
            BuildTime,
            ApiLarge,
            PublicUrl,
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
                            "version" => Ok(GeneratedField::Version),
                            "buildTime" | "build_time" => Ok(GeneratedField::BuildTime),
                            "apiLarge" | "api_large" => Ok(GeneratedField::ApiLarge),
                            "publicUrl" | "public_url" => Ok(GeneratedField::PublicUrl),
                            _ => Ok(GeneratedField::__SkipField__),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Urls;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct com.vizroy.xosl.newsweb.raw.v1.Urls")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Urls, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut version__ = None;
                let mut build_time__ = None;
                let mut api_large__ = None;
                let mut public_url__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Version => {
                            if version__.is_some() {
                                return Err(serde::de::Error::duplicate_field("version"));
                            }
                            version__ = Some(map_.next_value()?);
                        }
                        GeneratedField::BuildTime => {
                            if build_time__.is_some() {
                                return Err(serde::de::Error::duplicate_field("buildTime"));
                            }
                            build_time__ = Some(map_.next_value()?);
                        }
                        GeneratedField::ApiLarge => {
                            if api_large__.is_some() {
                                return Err(serde::de::Error::duplicate_field("apiLarge"));
                            }
                            api_large__ = Some(map_.next_value()?);
                        }
                        GeneratedField::PublicUrl => {
                            if public_url__.is_some() {
                                return Err(serde::de::Error::duplicate_field("publicUrl"));
                            }
                            public_url__ = Some(map_.next_value()?);
                        }
                        GeneratedField::__SkipField__ => {
                            let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(Urls {
                    version: version__.unwrap_or_default(),
                    build_time: build_time__.unwrap_or_default(),
                    api_large: api_large__.unwrap_or_default(),
                    public_url: public_url__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("com.vizroy.xosl.newsweb.raw.v1.Urls", FIELDS, GeneratedVisitor)
    }
}
