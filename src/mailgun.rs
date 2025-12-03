use email_address::EmailAddress;
use mime::Mime;
use reqwest::multipart::Part;
use serde::Deserialize;

use crate::Error;

pub enum Region {
    EU,
    US,
}

impl Region {
    fn base_url(&self) -> &'static str {
        match self {
            Self::US => "https://api.mailgun.net/v3",
            Self::EU => "https://api.eu.mailgun.net/v3",
        }
    }
}

pub struct Attachment {
    pub attachment_type: AttachmentType,
    pub name: String,
    pub bytes: Vec<u8>,
    pub mime_type: Mime,
}

pub enum AttachmentType {
    Attachment,
    Inline,
}

#[allow(clippy::too_many_arguments)]
pub async fn send_mailgun(
    from_name: &str,
    from_email: &EmailAddress,
    to: &[EmailAddress],
    subject: String,
    text: String,
    html: Option<String>,
    region: &Region,
    attachments: Vec<Attachment>,
    domain: &str,
    api_key: &str,
) -> Result<SendResponse, Error> {
    let client = reqwest::Client::new();

    let mut form = reqwest::multipart::Form::new();

    form = form.text("from", format!("{} <{}>", from_name, from_email));
    form = form.text(
        "to",
        to.iter()
            .map(|address| address.to_string())
            .collect::<Vec<_>>()
            .join(","),
    );
    form = form.text("subject", subject);
    form = form.text("text", text);
    if let Some(html) = html {
        form = form.text("html", html);
    }

    for attachment in attachments {
        let field_name = match attachment.attachment_type {
            AttachmentType::Attachment => "attachment",
            AttachmentType::Inline => "inline",
        };

        let mut part = Part::bytes(attachment.bytes);
        part = part.file_name(attachment.name);
        // UNWRAP: Mime should round-trip within reqwest.
        part = part.mime_str(attachment.mime_type.as_ref()).unwrap();
        form = form.part(field_name, part);
    }

    let url = format!("{}/{}/messages", region.base_url(), domain,);

    let res = client
        .post(url)
        .basic_auth("api", Some(api_key))
        .multipart(form)
        .send()
        .await?
        .error_for_status()?;

    let parsed = res.json().await?;
    Ok(parsed)
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct SendResponse {
    pub message: String,
    pub id: String,
}
