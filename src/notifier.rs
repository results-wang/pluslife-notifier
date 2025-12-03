use email_address::EmailAddress;
use uuid::Uuid;

use crate::{
    Error,
    mailgun::{Attachment, Region, send_mailgun},
    messages::SubgroupResult,
    state::CompletedTest,
};

const SENDER_NAME: &str = "PlusLife Results";

pub async fn notify(
    sender_email: &EmailAddress,
    mailgun_domain: &str,
    mailgun_api_key: &str,
    completed_test: CompletedTest,
    recipient: EmailAddress,
) -> Result<(), Error> {
    let html = format!(
        r#"<h2>Your PlusLife results are in.</h2>

<p>Your overall result is: {}</p>
<p>Your subgroup results are:</p>
{}
"#,
        completed_test.overall,
        to_html_list(&completed_test.subgroup_results)
    );
    let text = format!(
        r#"Your PlusLife results are in.

Your overall result is: {}
Your subgroup results are:
{}
"#,
        completed_test.overall,
        to_markdown_list(&completed_test.subgroup_results),
    );

    let attachments = vec![Attachment {
        attachment_type: crate::mailgun::AttachmentType::Inline,
        name: "graph.png".to_string(),
        bytes: completed_test.graph_png,
        mime_type: mime::IMAGE_PNG,
    }];

    send_mailgun(
        SENDER_NAME,
        sender_email,
        &[recipient],
        "Your PlusLife Results".to_owned(),
        text,
        Some(html),
        &Region::EU,
        attachments,
        mailgun_domain,
        mailgun_api_key,
    )
    .await?;
    Ok(())
}

pub async fn notify_error(
    sender_email: &EmailAddress,
    mailgun_domain: &str,
    mailgun_api_key: &str,
    id: &Uuid,
    error: &str,
    recipient: EmailAddress,
) -> Result<(), Error> {
    send_mailgun(
        SENDER_NAME,
        sender_email,
        &[recipient],
        "Error getting PlusLife results".to_owned(),
        format!("Sorry, an error occurred notifying you of your PlusLife result: {}. Your request ID was {}", error, id),
        None,
        &Region::EU,
        Vec::new(),
        mailgun_domain,
        mailgun_api_key,
    ).await?;
    Ok(())
}

fn to_html_list(results: &[SubgroupResult]) -> String {
    let mut str = "<ul>\n".to_owned();
    for result in results {
        str.push_str("  <li><strong>");
        str.push_str(normalise_result_name(&result.name));
        str.push_str("</strong>: ");
        str.push_str(&result.result.to_string());
        str.push_str("</li>\n");
    }
    str.push_str("</ul>");
    str
}

fn to_markdown_list(results: &[SubgroupResult]) -> String {
    let mut str = String::new();
    for result in results {
        str.push_str(" * ");
        str.push_str(normalise_result_name(&result.name));
        str.push('\n');
    }
    str
}

fn normalise_result_name(name: &str) -> &str {
    if name == "IC" { "Control" } else { name }
}
