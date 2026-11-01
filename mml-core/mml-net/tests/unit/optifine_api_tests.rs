use super::*;

/// 真实下载页里的一行，属性统一用单引号
const ROW: &str = "<tr class='downloadLine downloadLinePreview'> <td class='colFile'>OptiFine HD U K2 pre5</td> <td class='colDownload'><a href=\"http://adfoc.us/serve/sitelinks/?id=1&url=http://optifine.net/adloadx?f=preview_OptiFine_26.2_HD_U_K2_pre5.jar&x=bc68\">Download</a></td> <td class='colMirror'><a href=\"http://optifine.net/adloadx?f=preview_OptiFine_26.2_HD_U_K2_pre5.jar\">(Mirror)</a></td> <td class='colForge'>Forge N/A</td> <td class='colDate'>20.10.2026</td> </tr>";

#[test]
fn parses_download_row() {
    let rows = download_rows(ROW);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.file, "OptiFine HD U K2 pre5");
    assert_eq!(row.forge, "Forge N/A");
    assert_eq!(row.date, "20.10.2026");
    assert!(
        row.mirror
            .as_ref()
            .unwrap()
            .ends_with("preview_OptiFine_26.2_HD_U_K2_pre5.jar")
    );
}

#[test]
fn skips_rows_without_download_line_class() {
    let html = "<tr><td>head</td></tr><tr class='downloadLine'><td class='colFile'>a</td></tr>";
    let rows = download_rows(html);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].file, "a");
}

#[test]
fn finds_download_link() {
    let html = "<span id='Download'><a href='downloadx?f=x.jar&x=abc'>dl</a></span>";
    assert_eq!(
        download_link(html).as_deref(),
        Some("downloadx?f=x.jar&x=abc")
    );
}

