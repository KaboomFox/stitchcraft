//! The differential's report as a web page (M8.14): `target/conformance/differential/index.html`, which the
//! differential's workflow publishes with the pictures beside it.
//!
//! **Why from the Markdown.** The report is Markdown, the job's summary, and a page is what a person opens
//! on the site. The page is that Markdown turned into HTML line by line, so it says nothing the report
//! doesn't: headings, list items, tables and pictures, with code and nothing else marked inside a line. The
//! report's links to pictures are relative to `target/conformance/`, and the page lies in `differential/`
//! under it, so they lose that folder's name.

/// The page of the report `markdown`.
pub fn html(markdown: &str) -> String {
    let mut out = String::from(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>Ink/Stitch differential</title>\n\
         <style>body{font-family:sans-serif;margin:16px;max-width:1900px;background:#fff;color:#202020}\
         img{max-width:100%;height:auto;border:1px solid #ccc}table{border-collapse:collapse;margin:8px 0}\
         td,th{border:1px solid #ccc;padding:2px 8px;text-align:left}code{background:#f2f2f2}</style>\n</head>\n<body>\n",
    );
    let mut list = false;
    let mut table = false;
    for line in markdown.lines() {
        let is_list = line.starts_with("- ");
        let is_row = line.starts_with('|');
        if list && !is_list {
            out.push_str("</ul>\n");
            list = false;
        }
        if table && !is_row {
            out.push_str("</table>\n");
            table = false;
        }
        if let Some(rest) = line.strip_prefix("## ") {
            out.push_str(&format!("<h2>{}</h2>\n", inline(rest)));
        } else if let Some(rest) = line.strip_prefix("# ") {
            out.push_str(&format!("<h1>{}</h1>\n", inline(rest)));
        } else if is_list {
            if !list {
                out.push_str("<ul>\n");
                list = true;
            }
            out.push_str(&format!("<li>{}</li>\n", inline(line.get(2..).unwrap_or_default())));
        } else if is_row {
            let cells: Vec<&str> = line.trim().trim_matches('|').split('|').map(str::trim).collect();
            if cells.iter().all(|cell| cell.chars().all(|c| c == '-')) {
                continue;
            }
            let tag = if table { "td" } else { "th" };
            if !table {
                out.push_str("<table>\n");
                table = true;
            }
            let row: String = cells.iter().map(|cell| format!("<{tag}>{}</{tag}>", inline(cell))).collect();
            out.push_str(&format!("<tr>{row}</tr>\n"));
        } else if let Some((alt, link)) = image(line) {
            let link = link.strip_prefix("differential/").unwrap_or(link);
            out.push_str(&format!("<p><a href=\"{0}\"><img src=\"{0}\" alt=\"{1}\" loading=\"lazy\"></a></p>\n", escape(link), escape(alt)));
        } else if !line.trim().is_empty() {
            out.push_str(&format!("<p>{}</p>\n", inline(line)));
        }
    }
    if list {
        out.push_str("</ul>\n");
    }
    if table {
        out.push_str("</table>\n");
    }
    out.push_str("</body>\n</html>\n");
    out
}

/// The alternative text and the link of a line that is a picture alone, `![alt](link)`.
fn image(line: &str) -> Option<(&str, &str)> {
    let rest = line.trim().strip_prefix("![")?.strip_suffix(')')?;
    rest.split_once("](")
}

/// `text` escaped, with each pair of backticks around code.
fn inline(text: &str) -> String {
    let mut out = String::new();
    for (k, part) in text.split('`').enumerate() {
        if k % 2 == 1 {
            out.push_str(&format!("<code>{}</code>", escape(part)));
        } else {
            out.push_str(&escape(part));
        }
    }
    out
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_says_what_the_report_says_in_html() {
        let markdown = "# Ink/Stitch differential\n\nStitchCraft at `abc`.\n\n## ❌ `ring` (a.toml)\n\n- ❌ size differ\n- excused: x < y\n\n\
                        | Measure | StitchCraft | Ink/Stitch | |\n|---|---|---|---|\n| size | 1 | 2 | ❌ |\n\n\
                        ![ring: both](differential/pictures/ring.png)\n";
        let page = html(markdown);
        assert!(page.starts_with("<!doctype html>"), "{page}");
        assert!(page.contains("<h1>Ink/Stitch differential</h1>\n<p>StitchCraft at <code>abc</code>.</p>"), "{page}");
        assert!(page.contains("<h2>❌ <code>ring</code> (a.toml)</h2>\n<ul>\n<li>❌ size differ</li>\n<li>excused: x &lt; y</li>\n</ul>"), "{page}");
        assert!(page.contains("<table>\n<tr><th>Measure</th><th>StitchCraft</th><th>Ink/Stitch</th><th></th></tr>\n<tr><td>size</td><td>1</td><td>2</td><td>❌</td></tr>\n</table>"), "{page}");
        // The picture's link loses the folder the page lies in.
        assert!(page.contains("<a href=\"pictures/ring.png\"><img src=\"pictures/ring.png\" alt=\"ring: both\""), "{page}");
        assert!(page.ends_with("</body>\n</html>\n"));
        // A list or a table at the end is closed.
        assert!(html("- a").contains("<li>a</li>\n</ul>\n</body>"));
        assert!(html("| a |\n|---|\n| b |").contains("<tr><td>b</td></tr>\n</table>\n</body>"));
    }
}
