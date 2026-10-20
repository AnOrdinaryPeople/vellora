//! `break-inside: avoid` and `break-before: page` (CSS Fragmentation 3) as the
//! paginator honours them: on items-table `<tbody>` groups, items tables,
//! flattened wrappers and top-level blocks, from `<style>` rules, inline
//! `style`, and the legacy `page-break-*` aliases.

use vellora_core::pdf::PdfPage;
use vellora_core::{blitz_engine, page_css, pagination};

/// A short page so a handful of rows already crosses a page boundary.
const SHORT_PAGE: &str = "@page { size: 210mm 120mm; margin: 10mm; }";

fn paginate(html: &str) -> pagination::Paginated {
    let doc = blitz_engine::lay_out(html);
    let page_box = page_css::parse_page_box(html);
    pagination::paginate(&doc, &page_box)
}

fn pages_with(paginated: &pagination::Paginated, needle: &str) -> Vec<usize> {
    paginated
        .pages
        .iter()
        .enumerate()
        .filter(|(_, page)| page_has(page, needle))
        .map(|(i, _)| i)
        .collect()
}

fn page_has(page: &PdfPage, needle: &str) -> bool {
    page.text_runs.iter().any(|run| run.text.contains(needle))
}

fn first_text(page: &PdfPage) -> String {
    page.text_runs
        .iter()
        .min_by(|a, b| a.origin_y.total_cmp(&b.origin_y))
        .map(|run| run.text.to_string())
        .unwrap_or_default()
}

/// An items table with `groups` tbody groups of `rows` rows each, labelled
/// `[G<g>R<r>]`, plus the given extra CSS.
fn grouped_table(css: &str, groups: usize, rows: usize) -> String {
    let mut body = String::new();
    for g in 1..=groups {
        body.push_str("<tbody class=\"meal\">");
        for r in 1..=rows {
            body.push_str(&format!("<tr><td>[G{g}R{r}]</td><td>item</td></tr>"));
        }
        body.push_str("</tbody>");
    }
    format!(
        "<!DOCTYPE html><html><head><style>{SHORT_PAGE} body {{ font-size: 14px; }} \
         td {{ padding: 4px; }} {css}</style></head><body>\
         <table><thead><tr><th>Food</th><th>Qty</th></tr></thead>{body}</table>\
         </body></html>"
    )
}

/// Indexes of groups whose rows land on more than one page.
fn split_groups(paginated: &pagination::Paginated, groups: usize, rows: usize) -> Vec<usize> {
    (1..=groups)
        .filter(|g| {
            let mut pages: Vec<usize> = (1..=rows)
                .flat_map(|r| pages_with(paginated, &format!("[G{g}R{r}]")))
                .collect();
            pages.sort_unstable();
            pages.dedup();
            pages.len() > 1
        })
        .collect()
}

#[test]
fn without_hints_a_tbody_group_crosses_a_page_boundary() {
    let paginated = paginate(&grouped_table("", 8, 4));
    assert!(paginated.report.page_count > 1);
    assert!(
        !split_groups(&paginated, 8, 4).is_empty(),
        "control: the fixture must split some group when nothing asks it not to"
    );
}

#[test]
fn break_inside_avoid_keeps_every_tbody_group_on_one_page() {
    let paginated = paginate(&grouped_table("tbody.meal { break-inside: avoid; }", 8, 4));
    assert!(paginated.report.page_count > 1);
    assert_eq!(split_groups(&paginated, 8, 4), Vec::<usize>::new());
    for (i, page) in paginated.pages.iter().enumerate() {
        assert!(
            page_has(page, "Food"),
            "page {} repeats the table header",
            i + 1
        );
    }
}

#[test]
fn legacy_page_break_inside_avoid_is_honoured() {
    let paginated = paginate(&grouped_table("tbody { page-break-inside: avoid }", 8, 4));
    assert_eq!(split_groups(&paginated, 8, 4), Vec::<usize>::new());
}

#[test]
fn a_group_taller_than_a_page_falls_back_to_rows_without_losing_any() {
    let paginated = paginate(&grouped_table("tbody.meal { break-inside: avoid; }", 1, 40));
    assert!(
        paginated.report.page_count > 1,
        "40 rows cannot fit one short page"
    );
    for r in 1..=40 {
        assert_eq!(
            pages_with(&paginated, &format!("[G1R{r}]")).len(),
            1,
            "row {r} is placed exactly once"
        );
    }
    for (i, page) in paginated.pages.iter().enumerate() {
        assert!(
            page_has(page, "Food"),
            "page {} repeats the table header",
            i + 1
        );
    }
}

#[test]
fn break_before_page_starts_a_fresh_page_without_a_leading_blank() {
    let html = format!(
        "<!DOCTYPE html><html><head><style>{SHORT_PAGE} .v {{ break-before: page; }}</style></head><body>\
         <h2 class=\"v\">Weekdays</h2><p>Monday to Friday</p>\
         <h2 class=\"v\">Weekend</h2><p>Saturday and Sunday</p>\
         </body></html>"
    );
    let paginated = paginate(&html);
    assert_eq!(paginated.report.page_count, 2);
    assert_eq!(pages_with(&paginated, "Weekdays"), vec![0]);
    assert_eq!(pages_with(&paginated, "Weekend"), vec![1]);
    assert!(first_text(&paginated.pages[1]).contains("Weekend"));
}

#[test]
fn inline_and_legacy_break_before_are_honoured() {
    let html = format!(
        "<!DOCTYPE html><html><head><style>{SHORT_PAGE}</style></head><body>\
         <p>Intro</p>\
         <p style=\"break-before: page\">Second</p>\
         <p style=\"page-break-before: always\">Third</p>\
         </body></html>"
    );
    let paginated = paginate(&html);
    assert_eq!(paginated.report.page_count, 3);
    assert_eq!(pages_with(&paginated, "Second"), vec![1]);
    assert_eq!(pages_with(&paginated, "Third"), vec![2]);
}

#[test]
fn break_before_on_a_tbody_starts_its_rows_on_a_new_page_with_the_header() {
    let paginated = paginate(&grouped_table(
        "tbody.meal + tbody.meal { break-before: page; }",
        3,
        2,
    ));
    assert_eq!(paginated.report.page_count, 3);
    for g in 1..=3 {
        assert_eq!(pages_with(&paginated, &format!("[G{g}R1]")), vec![g - 1]);
        assert!(page_has(&paginated.pages[g - 1], "Food"));
    }
}

#[test]
fn break_inside_avoid_keeps_a_wrapper_section_together() {
    let filler: String = (1..=9).map(|i| format!("<p>Filler {i}</p>")).collect();
    let section: String = (1..=5).map(|i| format!("<p>Kept {i}</p>")).collect();
    let html = |css: &str| {
        format!(
            "<!DOCTYPE html><html><head><style>{SHORT_PAGE} p {{ margin: 0 0 10px; font-size: 16px; }} {css}</style></head><body>\
             {filler}<section class=\"keep\"><h3>Together</h3>{section}</section>\
             </body></html>"
        )
    };
    let pages_of_kept = |paginated: &pagination::Paginated| {
        let mut pages: Vec<usize> = (1..=5)
            .flat_map(|i| pages_with(paginated, &format!("Kept {i}")))
            .chain(pages_with(paginated, "Together"))
            .collect();
        pages.sort_unstable();
        pages.dedup();
        pages
    };

    let control = paginate(&html(""));
    assert!(
        pages_of_kept(&control).len() > 1,
        "control: the section straddles a page boundary when nothing asks it not to"
    );
    let kept = paginate(&html(".keep { break-inside: avoid; }"));
    assert_eq!(pages_of_kept(&kept).len(), 1);
}

#[test]
fn an_unparseable_selector_matches_nothing_and_does_not_fail_the_render() {
    let paginated = paginate(&grouped_table("tbody:::nope { break-inside: avoid }", 8, 4));
    assert!(paginated.report.page_count > 1);
}

#[test]
fn an_empty_break_div_still_forces_its_page_break() {
    for markup in [
        "<p>One</p><div class=\"pb\"></div><p>Two</p>",
        "<section><p>One</p><div class=\"pb\"></div><p>Two</p></section>",
    ] {
        let html = format!(
            "<!DOCTYPE html><html><head><style>{SHORT_PAGE} .pb {{ page-break-before: always; }}</style></head><body>{markup}</body></html>"
        );
        let paginated = paginate(&html);
        assert_eq!(paginated.report.page_count, 2, "{markup}");
        assert_eq!(pages_with(&paginated, "One"), vec![0], "{markup}");
        assert_eq!(pages_with(&paginated, "Two"), vec![1], "{markup}");
    }
}

#[test]
fn a_group_that_falls_back_keeps_the_spacing_it_would_have_without_avoid() {
    let with_after = |css: &str| {
        grouped_table(css, 1, 40).replace(
            "</table>",
            "</table><p style=\"margin-top: 24px\">After</p>",
        )
    };
    let after_y = |html: &str| {
        let paginated = paginate(html);
        let last = paginated.pages.last().expect("pages");
        last.text_runs
            .iter()
            .find(|run| run.text.contains("After"))
            .map(|run| run.origin_y)
            .expect("After is on the last page")
    };
    let plain = after_y(&with_after(""));
    let kept = after_y(&with_after("tbody.meal { break-inside: avoid; }"));
    assert!(
        (plain - kept).abs() < 0.01,
        "After at {kept} with avoid, {plain} without"
    );
}

#[test]
fn a_forced_break_inside_an_avoid_group_wins() {
    let paginated = paginate(&grouped_table(
        "table { break-inside: avoid; } tbody.meal + tbody.meal { break-before: page; }",
        2,
        2,
    ));
    assert_eq!(paginated.report.page_count, 2);
    assert_eq!(pages_with(&paginated, "[G1R1]"), vec![0]);
    assert_eq!(pages_with(&paginated, "[G2R1]"), vec![1]);
}
