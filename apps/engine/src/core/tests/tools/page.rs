//! read_page: which addresses are public, and the text pulled out of a page.

use std::net::IpAddr;

use serde_json::json;

use crate::core::config::ReadPage;
use crate::core::tests::support::ctx;
use crate::runtime::tools::page::{html_text, html_title, is_public, tools};

fn ip(text: &str) -> IpAddr {
    text.parse().unwrap_or(IpAddr::from([0, 0, 0, 0]))
}

#[test]
fn only_public_addresses_pass() {
    for open in ["8.8.8.8", "104.18.2.1", "2606:4700::1111"] {
        assert!(is_public(ip(open)), "{open}");
    }
    for closed in [
        "127.0.0.1",
        "10.1.2.3",
        "172.16.0.1",
        "192.168.1.1",
        "169.254.169.254",
        "100.64.0.1",
        "0.0.0.0",
        "::1",
        "fd00::1",
        "fe80::1",
        "::ffff:127.0.0.1",
    ] {
        assert!(!is_public(ip(closed)), "{closed}");
    }
}

#[test]
fn the_text_of_a_page_leaves_out_scripts_styles_and_tags() {
    let html = "<html><head><title>Halo &amp; Co</title><style>p{}</style></head><body>\
                <script>var x = 1;</script><h1>Halo</h1><p>We build&nbsp;cameras.</p>\
                <div>Join <a href=\"/jobs\">us</a></div></body></html>";
    assert_eq!(html_title(html).as_deref(), Some("Halo & Co"));
    let text = html_text(html);
    assert_eq!(text, "Halo\nWe build cameras.\nJoin us");
}

#[tokio::test]
async fn a_local_or_odd_link_is_refused() {
    let built = tools(&ReadPage::default());
    let Some(tool) = built.first() else {
        unreachable!("one tool")
    };
    for url in [
        "http://127.0.0.1/",
        "http://localhost:8000/",
        "ftp://example.com/",
        "http://169.254.169.254/latest/meta-data",
        "https://user:pw@example.com/",
    ] {
        let out = tool.call(&ctx(), json!({"url": url})).await;
        assert!(out.is_err(), "{url} must be refused");
    }
}
