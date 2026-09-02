//! The resume page as served. The contact line's segments are glued to the
//! separators, so without explicit wrap points its longest run sets the whole
//! page's minimum width - wider than a phone.

mod common;

use common::{body_string, get, test_router};
use server::security::Hsts;

#[tokio::test]
async fn contact_line_offers_a_wrap_point_after_each_separator() {
    let html = body_string(get(test_router(Hsts::Off), "/resume").await).await;
    assert_eq!(
        html.matches(r#"<span class="sep">·</span><wbr>"#).count(),
        3,
        "{html}"
    );
}
