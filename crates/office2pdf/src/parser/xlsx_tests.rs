use super::*;
use crate::ir::*;

/// Helper: build a minimal XLSX as bytes with a single sheet.
fn build_xlsx_bytes(sheet_name: &str, cells: &[(&str, &str)]) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    {
        let sheet = book.get_sheet_mut(&0).unwrap();
        sheet.set_name(sheet_name);
        for &(coord, value) in cells {
            sheet.get_cell_mut(coord).set_value(value);
        }
    }
    let mut cursor = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut cursor).unwrap();
    cursor.into_inner()
}

/// Helper: build XLSX with multiple sheets.
fn build_xlsx_multi_sheet(sheets: &[(&str, &[(&str, &str)])]) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    // Remove the default sheet first
    for (i, &(name, cells)) in sheets.iter().enumerate() {
        if i == 0 {
            let sheet = book.get_sheet_mut(&0).unwrap();
            sheet.set_name(name);
            for &(coord, value) in cells {
                sheet.get_cell_mut(coord).set_value(value);
            }
        } else {
            let mut sheet = umya_spreadsheet::Worksheet::default();
            sheet.set_name(name);
            for &(coord, value) in cells {
                sheet.get_cell_mut(coord).set_value(value);
            }
            book.add_sheet(sheet).unwrap();
        }
    }
    let mut cursor = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut cursor).unwrap();
    cursor.into_inner()
}

/// Helper: extract SheetPage from Document by index.
fn get_sheet_page(doc: &Document, idx: usize) -> &SheetPage {
    match &doc.pages[idx] {
        Page::Sheet(sp) => sp,
        _ => panic!("Expected SheetPage at index {idx}"),
    }
}

/// Helper: get cell text from a TableCell.
fn cell_text(cell: &TableCell) -> String {
    cell.content
        .iter()
        .filter_map(|b| match b {
            Block::Paragraph(p) => Some(p.runs.iter().map(|r| r.text.as_str()).collect::<String>()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Helper: extract the first run's TextStyle from a cell.
fn first_run_style(cell: &TableCell) -> &TextStyle {
    match &cell.content[0] {
        Block::Paragraph(p) => &p.runs[0].style,
        _ => panic!("Expected Paragraph"),
    }
}

// ----- Basic parsing tests -----

#[test]
fn test_parse_single_cell() {
    let data = build_xlsx_bytes("Sheet1", &[("A1", "Hello")]);
    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    assert_eq!(doc.pages.len(), 1);
    let tp = get_sheet_page(&doc, 0);
    assert_eq!(tp.name, "Sheet1");
    assert_eq!(tp.table.rows.len(), 1);
    assert_eq!(tp.table.rows[0].cells.len(), 1);
    assert_eq!(cell_text(&tp.table.rows[0].cells[0]), "Hello");
}

#[test]
fn test_parse_multiple_cells() {
    let data = build_xlsx_bytes(
        "Data",
        &[("A1", "Name"), ("B1", "Age"), ("A2", "Alice"), ("B2", "30")],
    );
    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let tp = get_sheet_page(&doc, 0);
    assert_eq!(tp.table.rows.len(), 2);
    assert_eq!(tp.table.rows[0].cells.len(), 2);
    assert_eq!(cell_text(&tp.table.rows[0].cells[0]), "Name");
    assert_eq!(cell_text(&tp.table.rows[0].cells[1]), "Age");
    assert_eq!(cell_text(&tp.table.rows[1].cells[0]), "Alice");
    assert_eq!(cell_text(&tp.table.rows[1].cells[1]), "30");
}

#[test]
fn test_parse_empty_cells_in_grid() {
    // A1 filled, B1 empty, A2 empty, B2 filled → 2x2 grid with gaps
    let data = build_xlsx_bytes("Sheet1", &[("A1", "Top-Left"), ("B2", "Bottom-Right")]);
    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let tp = get_sheet_page(&doc, 0);
    assert_eq!(tp.table.rows.len(), 2);
    assert_eq!(tp.table.rows[0].cells.len(), 2);
    // A1 has content
    assert_eq!(cell_text(&tp.table.rows[0].cells[0]), "Top-Left");
    // B1 is empty
    assert_eq!(cell_text(&tp.table.rows[0].cells[1]), "");
    // A2 is empty
    assert_eq!(cell_text(&tp.table.rows[1].cells[0]), "");
    // B2 has content
    assert_eq!(cell_text(&tp.table.rows[1].cells[1]), "Bottom-Right");
}

#[test]
fn test_parse_numbers() {
    let data = build_xlsx_bytes("Numbers", &[("A1", "42"), ("B1", "3.14")]);
    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let tp = get_sheet_page(&doc, 0);
    assert_eq!(cell_text(&tp.table.rows[0].cells[0]), "42");
    assert_eq!(cell_text(&tp.table.rows[0].cells[1]), "3.14");
}

#[test]
fn test_parse_dates_as_text() {
    let data = build_xlsx_bytes("Dates", &[("A1", "2024-01-15"), ("A2", "December 25")]);
    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let tp = get_sheet_page(&doc, 0);
    assert_eq!(cell_text(&tp.table.rows[0].cells[0]), "2024-01-15");
    assert_eq!(cell_text(&tp.table.rows[1].cells[0]), "December 25");
}

// ----- Sheet name tests -----

#[test]
fn test_sheet_name_preserved() {
    let data = build_xlsx_bytes("Financial Report", &[("A1", "Revenue")]);
    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let tp = get_sheet_page(&doc, 0);
    assert_eq!(tp.name, "Financial Report");
}

// ----- Multi-sheet tests -----

#[test]
fn test_parse_multiple_sheets() {
    let data = build_xlsx_multi_sheet(&[
        ("Sheet1", &[("A1", "Data1")]),
        ("Sheet2", &[("A1", "Data2")]),
    ]);
    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    assert_eq!(doc.pages.len(), 2);
    let tp1 = get_sheet_page(&doc, 0);
    let tp2 = get_sheet_page(&doc, 1);
    assert_eq!(tp1.name, "Sheet1");
    assert_eq!(tp2.name, "Sheet2");
    assert_eq!(cell_text(&tp1.table.rows[0].cells[0]), "Data1");
    assert_eq!(cell_text(&tp2.table.rows[0].cells[0]), "Data2");
}

// ----- Column width tests -----

#[test]
fn test_column_widths_default() {
    let data = build_xlsx_bytes("Sheet1", &[("A1", "Hello"), ("B1", "World")]);
    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let tp = get_sheet_page(&doc, 0);
    assert_eq!(tp.table.column_widths.len(), 2);
    // umya's writer auto-emits `<col min="1" max="2" width="8.38"
    // customWidth="1"/>` for used columns, so this fixture takes the
    // declared-width path: round(8.38 × 6pt Calibri-11 unit) = 50pt on the
    // integer point grid (issue #621; the old pixel model printed 50.28pt).
    for w in &tp.table.column_widths {
        assert_eq!(
            *w, 50.0,
            "Expected declared 8.38-unit width of 50pt, got {w}"
        );
    }
}

#[test]
fn test_carlito_column_widths_match_native_print_metrics() {
    // Carlito 11 has a 6pt column unit (issue #621), so the pr_186 fixture's
    // native 26/20/24-unit columns print 156/120/144pt.
    assert_eq!(column_width_to_pt(26.0, 6.0), 156.0);
    assert_eq!(column_width_to_pt(20.0, 6.0), 120.0);
    assert_eq!(column_width_to_pt(24.0, 6.0), 144.0);
}

#[test]
fn test_sheet_uses_dominant_carlito_font_for_column_metrics() {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.get_sheet_mut(&0).unwrap();
    sheet
        .get_cell_mut("A1")
        .set_value("Header")
        .get_style_mut()
        .get_font_mut()
        .set_name("Carlito");
    sheet
        .get_cell_mut("A2")
        .set_value("Body")
        .get_style_mut()
        .get_font_mut()
        .set_name("Carlito");

    // Styles-unreadable fallback: the dominant Carlito face at the assumed
    // 11pt Normal size gives the same 6pt unit as a declared Carlito-11
    // Normal font (issue #621).
    assert_eq!(sheet_column_unit_pt(sheet), 6.0);
}

/// The column character-unit is an INTEGER POINT count: round-half-up of the
/// Normal font's max digit advance in points. Measured on 17 one-factor
/// native Excel-for-Mac probes (issue #621): each family/size pair below is a
/// discriminator — Calibri 10 → 5pt kills every integer-96dpi-pixel model
/// (the old ceil gave 7px = 5.25pt), Times New Roman 13 (exactly 6.500pt)
/// rounds UP to 7 (kills half-even), Calibri 9 and Verdana 11 kill
/// truncation, Calibri 10 and Verdana 10 kill ceiling.
#[test]
fn test_column_unit_pt_is_integer_points_from_digit_advance() {
    assert_eq!(column_unit_pt("Calibri", 9.0), 5.0);
    assert_eq!(column_unit_pt("Calibri", 10.0), 5.0);
    assert_eq!(column_unit_pt("Calibri", 11.0), 6.0);
    assert_eq!(column_unit_pt("Calibri", 12.0), 6.0);
    assert_eq!(column_unit_pt("Arial", 10.0), 6.0);
    assert_eq!(column_unit_pt("Arial", 12.0), 7.0);
    assert_eq!(column_unit_pt("Verdana", 10.0), 6.0);
    assert_eq!(column_unit_pt("Verdana", 11.0), 7.0);
    assert_eq!(column_unit_pt("Times New Roman", 12.0), 6.0);
    assert_eq!(column_unit_pt("Times New Roman", 13.0), 7.0);
    assert_eq!(column_unit_pt("Courier New", 10.0), 6.0);
    assert_eq!(column_unit_pt("Courier New", 12.0), 7.0);
    assert_eq!(column_unit_pt("Malgun Gothic", 10.0), 6.0);
    assert_eq!(column_unit_pt("Malgun Gothic", 11.0), 6.0);
}

/// The reference digit advances are the real `hmtx` maxima over U+0030..=0039
/// of the faces Excel itself resolves (read from Excel's own DFonts/system
/// faces by the issue #621 probe tooling). They pin the wasm/font-less arm so
/// output stays deterministic, and they outrank live font resolution so a
/// machine substituting a digit-incompatible face (Calibri → Liberation Sans
/// is 0.556em against Calibri's 0.5068) cannot shift column geometry.
#[test]
fn test_reference_digit_advance_em_pins_excel_face_metrics() {
    let calibri: f64 = reference_digit_advance_em("Calibri").unwrap();
    assert!((calibri - 0.506836).abs() < 1e-6);
    assert_eq!(
        reference_digit_advance_em("Carlito"),
        reference_digit_advance_em("Calibri"),
        "Carlito is metrically identical to Calibri"
    );
    let arial: f64 = reference_digit_advance_em("Arial").unwrap();
    assert!((arial - 0.556152).abs() < 1e-6);
    let verdana: f64 = reference_digit_advance_em("Verdana").unwrap();
    assert!((verdana - 0.635742).abs() < 1e-6);
    let times: f64 = reference_digit_advance_em("Times New Roman").unwrap();
    assert!((times - 0.500000).abs() < 1e-6);
    let courier: f64 = reference_digit_advance_em("Courier New").unwrap();
    assert!((courier - 0.600098).abs() < 1e-6);
    // The repo's previous 0.529em Malgun estimate was wrong: the real face
    // advances 0.550781em (issue #621 probe artifacts).
    let malgun: f64 = reference_digit_advance_em("Malgun Gothic").unwrap();
    assert!((malgun - 0.550781).abs() < 1e-6);
    assert_eq!(
        reference_digit_advance_em("맑은 고딕"),
        reference_digit_advance_em("Malgun Gothic"),
        "the localized Malgun name must map to the same face"
    );
    assert_eq!(
        reference_digit_advance_em("Definitely Not A Font"),
        None,
        "unknown families fall through to live font resolution"
    );
}

/// Families outside the reference table resolve their digit advance from the
/// real face `hmtx`, exactly as Excel measures the face it resolves. The
/// embedded Libertinus Serif face makes this deterministic on every target:
/// its digit advance is 465/1000 em.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_max_digit_advance_em_reads_real_face_hmtx() {
    let advance: f64 = crate::render::pdf::max_digit_advance_em("Libertinus Serif")
        .expect("the embedded Libertinus Serif face must resolve");
    assert!(
        (advance - 0.465).abs() < 1e-6,
        "Libertinus Serif digit advance should be 0.465em, got {advance}"
    );
    // And the column metric consumes it: round(0.465 × 11pt) = 5pt.
    assert_eq!(column_unit_pt("Libertinus Serif", 11.0), 5.0);
}

/// A declared column width prints as an integer point count: Excel quantizes
/// `width × unit` per column. Probe calibri11frac (issue #621): width 10.6 at
/// the 6pt Calibri-11 unit prints 64pt, not 63.6pt.
#[test]
fn test_declared_column_width_quantizes_to_integer_points() {
    assert_eq!(column_width_to_pt(10.6, 6.0), 64.0);
    // Whole-unit widths land on exact multiples — the pr_186 fixture's
    // Carlito-11 26/20/24-unit columns stay 156/120/144pt.
    assert_eq!(column_width_to_pt(26.0, 6.0), 156.0);
    assert_eq!(column_width_to_pt(20.0, 6.0), 120.0);
    assert_eq!(column_width_to_pt(24.0, 6.0), 144.0);
}

/// A column with no `<col>` entry and no declared `defaultColWidth` prints at
/// `baseColWidth × unit + 5` points — NOT 8.43 character units — where
/// `baseColWidth` defaults to 8 when `sheetFormatPr` does not declare it.
/// Verified by the issue #621 probes: at the 6pt Calibri-11 unit,
/// baseColWidth 10 → 65pt and 12 → 77pt (round-3 probes calibri11base10/12),
/// absent → 53pt; units 5/7 with no baseColWidth → 45/61pt. A declared
/// `defaultColWidth` outranks `baseColWidth` and goes through the
/// declared-units quantization instead.
#[test]
fn test_default_column_width_is_base_col_width_units_plus_five_points() {
    assert_eq!(default_column_width_pt(None, None, 5.0), 45.0);
    assert_eq!(default_column_width_pt(None, None, 6.0), 53.0);
    assert_eq!(default_column_width_pt(None, None, 7.0), 61.0);
    // Measured baseColWidth probes (no defaultColWidth): 10 → 65, 12 → 77.
    assert_eq!(default_column_width_pt(None, Some(10), 6.0), 65.0);
    assert_eq!(default_column_width_pt(None, Some(12), 6.0), 77.0);
    // Declared defaultColWidth quantizes like any declared width and
    // outranks baseColWidth.
    assert_eq!(default_column_width_pt(Some(10.6), None, 6.0), 64.0);
    assert_eq!(default_column_width_pt(Some(10.6), Some(12), 6.0), 64.0);
}

/// `declared_base_column_width` surfaces `sheetFormatPr@baseColWidth` only
/// when the file declares one — umya reports 0 for an absent attribute,
/// a width Excel never writes.
#[test]
fn test_declared_base_column_width_reads_sheet_format_pr() {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.get_sheet_mut(&0).unwrap();
    assert_eq!(declared_base_column_width(sheet), None);
    sheet
        .get_sheet_format_properties_mut()
        .set_base_column_width(10);
    assert_eq!(declared_base_column_width(sheet), Some(10));
}

#[test]
fn test_extract_normal_font_reads_first_styles_font() {
    let mut book = umya_spreadsheet::new_file();
    book.get_sheet_mut(&0)
        .unwrap()
        .get_cell_mut("A1")
        .set_value("x");
    let mut cursor = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut cursor).unwrap();
    let data = cursor.into_inner();

    let normal_font = extract_normal_font(&data).expect("styles.xml has a Normal font");
    assert_eq!(normal_font.family, "Calibri");
    assert_eq!(normal_font.size_pt, 11.0);
}

#[test]
fn test_column_overflow_splits_to_second_page_like_excel() {
    // Quotation-style layout: A4 portrait with 0.75in side margins leaves a
    // 487pt printable width. Columns of 5+30+16+8+14+16 = 89 chars under the
    // Calibri-11 Normal font are 534pt at Excel's 8px MDW, so the last
    // column overflows onto page 2 — exactly how Excel paginates the audit
    // fixture (issue #366).
    let mut book = umya_spreadsheet::new_file();
    {
        let sheet = book.get_sheet_mut(&0).unwrap();
        sheet.set_name("Sheet1");
        for (index, (col, width)) in [
            ("A", 5.0),
            ("B", 30.0),
            ("C", 16.0),
            ("D", 8.0),
            ("E", 14.0),
            ("F", 16.0),
        ]
        .iter()
        .enumerate()
        {
            sheet.get_column_dimension_mut(col).set_width(*width);
            let cell_ref = format!("{}1", col);
            sheet
                .get_cell_mut(cell_ref.as_str())
                .set_value(format!("Col {}", index + 1));
        }
        let margins = sheet.get_page_margins_mut();
        margins.set_left(0.75);
        margins.set_right(0.75);
        margins.set_top(1.0);
        margins.set_bottom(1.0);
    }
    let mut cursor = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut cursor).unwrap();

    let parser = XlsxParser;
    let (doc, _warnings) = parser
        .parse(&cursor.into_inner(), &ConvertOptions::default())
        .unwrap();
    assert_eq!(
        doc.pages.len(),
        2,
        "the sixth column must overflow onto its own page like Excel"
    );
}

// ----- Page size and margins defaults -----

#[test]
fn test_page_size_defaults() {
    let data = build_xlsx_bytes("Sheet1", &[("A1", "Test")]);
    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let tp = get_sheet_page(&doc, 0);
    let default_size = PageSize::default();
    assert!((tp.size.width - default_size.width).abs() < 0.01);
    assert!((tp.size.height - default_size.height).abs() < 0.01);
}

/// Build a workbook whose only sheet has no cells, carrying a paper size and a
/// header/footer. LibreOffice writes exactly this shape for a workbook saved
/// with nothing typed into it.
///
/// The header and footer are declared so the tests can assert they are *not*
/// carried onto the blank page, not because the page renders them.
fn build_empty_sheet_xlsx(paper_size: u32, header: &str, footer: &str) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    {
        let sheet = book.get_sheet_mut(&0).unwrap();
        sheet.get_page_setup_mut().set_paper_size(paper_size);
        sheet
            .get_header_footer_mut()
            .get_odd_header_mut()
            .set_value(header);
        sheet
            .get_header_footer_mut()
            .get_odd_footer_mut()
            .set_value(footer);
    }
    let mut cursor = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut cursor).unwrap();
    cursor.into_inner()
}

/// A workbook whose only sheet has no cells still prints one page, and that
/// page is the size the sheet asks for.
///
/// The sheet loop skips a sheet with no used range, so a single-sheet workbook
/// reached codegen with no pages at all and the compiler's own default supplied
/// a blank A4 — the file's `<pageSetup paperSize="1"/>` never reached the
/// renderer (issue #632).
#[test]
fn test_empty_sheet_keeps_its_declared_paper_size() {
    // 1 = Letter.
    let data = build_empty_sheet_xlsx(1, "&CReport", "&CPage &P");
    let (doc, _warnings) = XlsxParser.parse(&data, &ConvertOptions::default()).unwrap();

    assert_eq!(doc.pages.len(), 1, "an empty sheet still prints one page");
    let page = get_sheet_page(&doc, 0);
    assert!(
        (page.size.width - 612.0).abs() < 0.01 && (page.size.height - 792.0).abs() < 0.01,
        "expected Letter, got {:?}",
        page.size
    );
}

/// Triangulation: a different paper code must produce that code's size, so the
/// page cannot be a hardcoded Letter.
#[test]
fn test_empty_sheet_keeps_a_non_letter_paper_size() {
    // 5 = Legal.
    let data = build_empty_sheet_xlsx(5, "&CReport", "&CPage &P");
    let (doc, _warnings) = XlsxParser.parse(&data, &ConvertOptions::default()).unwrap();

    let page = get_sheet_page(&doc, 0);
    assert!(
        (page.size.width - 612.0).abs() < 0.01 && (page.size.height - 1008.0).abs() < 0.01,
        "expected Legal, got {:?}",
        page.size
    );
}

/// The page an empty sheet prints stays blank.
///
/// The ground truth for a sheet with no used range is a blank page — Excel
/// declines to print one at all — so nothing is invented to fill it. Only the
/// paper the file asks for is restored.
#[test]
fn test_empty_sheet_page_stays_blank() {
    let data = build_empty_sheet_xlsx(1, "&CQuarterly", "&CPage &P");
    let (doc, _warnings) = XlsxParser.parse(&data, &ConvertOptions::default()).unwrap();

    let page = get_sheet_page(&doc, 0);
    assert!(page.header.is_none(), "no header on a page with no cells");
    assert!(page.footer.is_none(), "no footer on a page with no cells");
    assert!(page.table.rows.is_empty());
    assert!(page.images.is_empty() && page.charts.is_empty());
}

/// The page still carries the sheet's own print margins, not the renderer's.
#[test]
fn test_empty_sheet_page_keeps_its_print_margins() {
    let mut book = umya_spreadsheet::new_file();
    {
        let sheet = book.get_sheet_mut(&0).unwrap();
        sheet.get_page_setup_mut().set_paper_size(1);
        sheet.get_page_margins_mut().set_left(1.25);
    }
    let mut cursor = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut cursor).unwrap();

    let (doc, _warnings) = XlsxParser
        .parse(&cursor.into_inner(), &ConvertOptions::default())
        .unwrap();

    let page = get_sheet_page(&doc, 0);
    assert!(
        (page.margins.left - 90.0).abs() < 0.01,
        "expected 1.25in = 90pt, got {}",
        page.margins.left
    );
}

/// A sheet that does have cells keeps deciding the page count on its own — an
/// empty *second* sheet must not add a blank page, which is what Excel does.
#[test]
fn test_empty_sheet_alongside_a_used_sheet_adds_no_page() {
    let mut book = umya_spreadsheet::new_file();
    {
        let sheet = book.get_sheet_mut(&0).unwrap();
        sheet.set_name("Data");
        sheet.get_cell_mut("A1").set_value("Value");
    }
    book.new_sheet("Blank").unwrap();
    let mut cursor = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut cursor).unwrap();

    let (doc, _warnings) = XlsxParser
        .parse(&cursor.into_inner(), &ConvertOptions::default())
        .unwrap();

    assert_eq!(doc.pages.len(), 1, "the blank sheet contributes no page");
    assert_eq!(get_sheet_page(&doc, 0).name, "Data");
}

// ----- Table structure tests -----

#[test]
fn test_table_row_column_consistency() {
    // 3x3 grid, only some cells filled
    let data = build_xlsx_bytes(
        "Grid",
        &[("A1", "1"), ("C1", "3"), ("B2", "5"), ("C3", "9")],
    );
    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let tp = get_sheet_page(&doc, 0);
    assert_eq!(tp.table.rows.len(), 3, "Expected 3 rows");
    // All rows should have same number of columns
    for row in &tp.table.rows {
        assert_eq!(row.cells.len(), 3, "Expected 3 columns per row");
    }
}

// ----- Error handling -----

#[test]
fn test_parse_invalid_data_returns_error() {
    let parser = XlsxParser;
    let result = parser.parse(b"not an xlsx file", &ConvertOptions::default());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, ConvertError::Parse(_)),
        "Expected Parse error, got {err:?}"
    );
}

#[test]
fn test_parse_error_includes_library_name() {
    let parser = XlsxParser;
    let result = parser.parse(b"not an xlsx file", &ConvertOptions::default());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("umya-spreadsheet"),
        "Parse error should include upstream library name 'umya-spreadsheet', got: {msg}"
    );
}

// ----- Empty cell content -----

#[test]
fn test_empty_cells_have_no_content() {
    let data = build_xlsx_bytes("Sheet1", &[("A1", "Only A1"), ("C1", "Only C1")]);
    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let tp = get_sheet_page(&doc, 0);
    // B1 should be empty (no paragraphs)
    assert!(
        tp.table.rows[0].cells[1].content.is_empty(),
        "Expected empty cell content for B1"
    );
}

#[test]
fn test_cell_default_span_values() {
    let data = build_xlsx_bytes("Sheet1", &[("A1", "Test")]);
    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let tp = get_sheet_page(&doc, 0);
    let cell = &tp.table.rows[0].cells[0];
    assert_eq!(cell.col_span, 1);
    assert_eq!(cell.row_span, 1);
    assert!(cell.border.is_none());
    assert!(cell.background.is_none());
}

#[path = "xlsx_cell_format_tests.rs"]
mod cell_format_tests;

#[path = "xlsx_page_feature_tests.rs"]
mod page_feature_tests;

#[path = "xlsx_condfmt_tests.rs"]
mod condfmt_tests;

#[path = "xlsx_chart_tests.rs"]
mod chart_tests;

#[path = "xlsx_streaming_tests.rs"]
mod streaming_tests;

/// The style of the first run of the first cell a parsed workbook produces.
fn first_cell_text_style(data: &[u8]) -> crate::ir::TextStyle {
    let (doc, _warnings) = XlsxParser
        .parse(data, &ConvertOptions::default())
        .expect("workbook should parse");
    let Page::Sheet(sheet) = &doc.pages[0] else {
        panic!("expected a sheet page");
    };
    for row in &sheet.table.rows {
        for cell in &row.cells {
            for block in &cell.content {
                if let Block::Paragraph(paragraph) = block
                    && let Some(run) = paragraph.runs.first()
                {
                    return run.style.clone();
                }
            }
        }
    }
    panic!("expected a cell run");
}

#[test]
fn test_unstyled_cell_carries_the_workbook_normal_font() {
    // A cell with no `s` attribute uses cellXfs[0], whose font is the
    // workbook's Normal font. umya reports no font for such a cell, so the
    // style path has to fall back to styles.xml itself or the renderer picks
    // its own default family and size (issue #462).
    let data = build_xlsx_with_normal_font("Malgun Gothic", 12.0);
    let style = first_cell_text_style(&data);
    assert_eq!(style.font_family.as_deref(), Some("Malgun Gothic"));
    assert_eq!(style.font_size, Some(12.0));
}

#[test]
fn test_unstyled_cell_keeps_a_calibri_normal_font() {
    // Triangulation: Calibri is the most common Normal font and used to be
    // dropped on the grounds that it was "the default". It has to survive
    // like any other family, or Calibri workbooks render in the renderer's
    // serif default (issue #462).
    let data = build_xlsx_with_normal_font("Calibri", 11.0);
    let style = first_cell_text_style(&data);
    assert_eq!(style.font_family.as_deref(), Some("Calibri"));
    assert_eq!(style.font_size, Some(11.0));
}

#[test]
fn test_explicit_cell_font_overrides_the_workbook_normal_font() {
    let mut book = umya_spreadsheet::new_file();
    {
        let sheet = book.get_sheet_mut(&0).unwrap();
        let cell = sheet.get_cell_mut("A1");
        cell.set_value("styled");
        cell.get_style_mut().get_font_mut().set_name("Georgia");
        cell.get_style_mut().get_font_mut().set_size(20.0);
    }
    let mut cursor = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut cursor).unwrap();
    let style = first_cell_text_style(&cursor.into_inner());
    assert_eq!(style.font_family.as_deref(), Some("Georgia"));
    assert_eq!(style.font_size, Some(20.0));
}

/// A one-cell workbook whose Normal font (styles.xml font 0) is `family` at
/// `size_pt`, with the cell itself left unstyled.
fn build_xlsx_with_normal_font(family: &str, size_pt: f64) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    {
        let sheet = book.get_sheet_mut(&0).unwrap();
        sheet.get_cell_mut("A1").set_value("title");
    }
    let mut cursor = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut cursor).unwrap();
    rewrite_first_styles_font(&cursor.into_inner(), family, size_pt)
}

/// Rewrite the first `<font>` of `xl/styles.xml` in place. umya always
/// writes Calibri 11 there, so the fixture has to patch the part directly to
/// exercise a different Normal font.
fn rewrite_first_styles_font(data: &[u8], family: &str, size_pt: f64) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(data)).expect("readable zip");
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("readable entry");
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes).expect("readable entry body");
        if name == "xl/styles.xml" {
            let xml = String::from_utf8(bytes).expect("styles.xml is utf-8");
            let start = xml.find("<font>").expect("styles.xml has a font");
            let end = xml[start..].find("</font>").expect("font is closed") + start;
            let replacement =
                format!("<font><sz val=\"{size_pt}\"/><name val=\"{family}\"/></font>");
            bytes = format!(
                "{}{}{}",
                &xml[..start],
                replacement,
                &xml[end + "</font>".len()..]
            )
            .into_bytes();
        }
        out.start_file(name, zip::write::SimpleFileOptions::default())
            .expect("writable entry");
        std::io::Write::write_all(&mut out, &bytes).expect("writable entry body");
    }
    out.finish().expect("finished zip").into_inner()
}

// ----- Drawing-only sheets (issue #620) -----

/// A workbook whose only sheet has no cells but carries one picture anchored
/// C1:F9 (cols 2..5, rows 0..8, zero offsets). umya cannot author drawings,
/// so the drawing parts are spliced into the zip it writes.
fn build_drawing_only_sheet_xlsx() -> Vec<u8> {
    let book = umya_spreadsheet::new_file();
    let mut cursor = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut cursor).unwrap();
    splice_picture_drawing(&cursor.into_inner())
}

/// Splice `xl/drawings/drawing1.xml` (one twoCellAnchor picture), its rels,
/// and a 1x1 PNG into a workbook zip, wiring the first worksheet to it.
fn splice_picture_drawing(data: &[u8]) -> Vec<u8> {
    const DRAWING_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><xdr:twoCellAnchor><xdr:from><xdr:col>2</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>5</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>8</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to><xdr:pic><xdr:nvPicPr><xdr:cNvPr id="2" name="Picture 1"/><xdr:cNvPicPr/></xdr:nvPicPr><xdr:blipFill><a:blip r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill><xdr:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic><xdr:clientData/></xdr:twoCellAnchor></xdr:wsDr>"#;
    const DRAWING_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/></Relationships>"#;
    const SHEET_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDrawing1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/></Relationships>"#;
    /// Smallest valid PNG: 1x1 RGBA.
    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x62, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    let mut archive = zip::ZipArchive::new(Cursor::new(data)).expect("readable zip");
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut has_sheet_rels = false;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("readable entry");
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes).expect("readable entry body");
        match name.as_str() {
            "xl/worksheets/sheet1.xml" => {
                let xml = String::from_utf8(bytes).expect("sheet1.xml is utf-8");
                bytes = xml
                    .replace(
                        "</worksheet>",
                        r#"<drawing r:id="rIdDrawing1"/></worksheet>"#,
                    )
                    .into_bytes();
            }
            "xl/worksheets/_rels/sheet1.xml.rels" => {
                has_sheet_rels = true;
                let xml = String::from_utf8(bytes).expect("sheet rels is utf-8");
                bytes = xml
                    .replace(
                        "</Relationships>",
                        r#"<Relationship Id="rIdDrawing1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/></Relationships>"#,
                    )
                    .into_bytes();
            }
            "[Content_Types].xml" => {
                let xml = String::from_utf8(bytes).expect("content types is utf-8");
                bytes = xml
                    .replace(
                        "</Types>",
                        r#"<Default Extension="png" ContentType="image/png"/><Override PartName="/xl/drawings/drawing1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/></Types>"#,
                    )
                    .into_bytes();
            }
            _ => {}
        }
        out.start_file(name, zip::write::SimpleFileOptions::default())
            .expect("writable entry");
        std::io::Write::write_all(&mut out, &bytes).expect("writable entry body");
    }
    if !has_sheet_rels {
        out.start_file(
            "xl/worksheets/_rels/sheet1.xml.rels",
            zip::write::SimpleFileOptions::default(),
        )
        .expect("writable sheet rels");
        std::io::Write::write_all(&mut out, SHEET_RELS.as_bytes()).expect("writable sheet rels");
    }
    for (path, body) in [
        ("xl/drawings/drawing1.xml", DRAWING_XML.as_bytes()),
        (
            "xl/drawings/_rels/drawing1.xml.rels",
            DRAWING_RELS.as_bytes(),
        ),
        ("xl/media/image1.png", PNG_1X1),
    ] {
        out.start_file(path, zip::write::SimpleFileOptions::default())
            .expect("writable drawing part");
        std::io::Write::write_all(&mut out, body).expect("writable drawing part body");
    }
    out.finish().expect("finished zip").into_inner()
}

/// A sheet with no cells must resolve its drawing anchors against the
/// workbook Normal font, producing the same column metric as a populated
/// sheet (issue #620). umya writes Calibri 11 as the Normal font, whose 6pt
/// unit prices an undeclared default column at 8 × 6 + 5 = 53pt (issue #621
/// probes); the legacy hardcoded 7px metric produced 44.2575pt.
#[test]
fn test_drawing_only_sheet_resolves_anchors_with_normal_font_metric() {
    let data = build_drawing_only_sheet_xlsx();
    let (doc, _warnings) = XlsxParser.parse(&data, &ConvertOptions::default()).unwrap();

    let page = get_sheet_page(&doc, 0);
    assert_eq!(
        page.images.len(),
        1,
        "the spliced picture must survive parse"
    );
    let image: &crate::ir::SheetImage = &page.images[0];

    let column_pt: f64 = default_column_width_pt(None, None, 6.0);
    assert_eq!(
        column_pt, 53.0,
        "Calibri-11 default column must be 53pt, got {column_pt}"
    );
    // Anchor spans cols 2..5 with zero offsets: x = 2 columns, width = 3.
    assert!(
        (image.x_offset_pt - 2.0 * column_pt).abs() < 0.01,
        "x_offset_pt {} != 2 x {column_pt}",
        image.x_offset_pt
    );
    let width: f64 = image.image.width.expect("twoCellAnchor resolves a width");
    assert!(
        (width - 3.0 * column_pt).abs() < 0.01,
        "width {width} != 3 x {column_pt}"
    );
    // Negative: neither the legacy hardcoded-7px metric (44.2575pt columns)
    // nor the pre-#621 8.43-character model (50.58pt) may resurface.
    for stale_column_pt in [44.2575_f64, 50.58_f64] {
        assert!(
            (width - 3.0 * stale_column_pt).abs() > 1.0,
            "width {width} still matches the stale {stale_column_pt}pt column metric"
        );
    }
}

/// Triangulation for issue #620: the empty-sheet context must derive its
/// metric from whatever Normal font it is given — not a hardcoded value —
/// and fall back to the legacy 5.25pt unit only when no Normal font is
/// readable. The carried `normal_font` keeps the stub structurally
/// consistent with a populated-sheet context; nothing on the drawing-only
/// path reads it today (text boxes take their fonts from DrawingML run
/// properties and the theme).
#[test]
fn test_empty_sheet_context_derives_metric_from_normal_font() {
    let book = umya_spreadsheet::new_file();
    let sheet: &umya_spreadsheet::Worksheet = book.get_sheet(&0).unwrap();

    let calibri_11 = NormalFont {
        family: "Calibri".to_string(),
        size_pt: 11.0,
    };
    let calibri_ctx = empty_sheet_context(sheet, Some(&calibri_11));
    assert_eq!(resolve_column_unit_pt(sheet, Some(&calibri_11)), 6.0);
    assert_eq!(calibri_ctx.default_column_width_pt, 53.0);
    assert_eq!(calibri_ctx.normal_font, Some(calibri_11));

    // A smaller Normal font must shrink the metric with it:
    // round(0.506836 × 8pt) = 4pt unit → 8 × 4 + 5 = 37pt default columns
    // (issue #621 model).
    let calibri_8 = NormalFont {
        family: "Calibri".to_string(),
        size_pt: 8.0,
    };
    assert_eq!(resolve_column_unit_pt(sheet, Some(&calibri_8)), 4.0);
    assert_eq!(
        empty_sheet_context(sheet, Some(&calibri_8)).default_column_width_pt,
        37.0
    );

    // No readable Normal font: the shared cell-font fallback finds no cells
    // on an empty sheet and keeps the legacy 5.25pt unit (7px × 0.75); the
    // #621 probes never covered a stylesheet-less workbook.
    let fallback_ctx = empty_sheet_context(sheet, None);
    assert_eq!(resolve_column_unit_pt(sheet, None), 5.25);
    assert_eq!(fallback_ctx.default_column_width_pt, 8.0 * 5.25 + 5.0);
    assert_eq!(fallback_ctx.normal_font, None);
}

/// Rewrite the workbook's worksheet parts, inserting `insertion` before each
/// closing `</worksheet>` tag. umya's writer does not model
/// `printOptions@gridLines`, so the attribute is injected into the archive
/// the way Excel writes it — after `sheetData`.
fn inject_before_worksheet_close(xlsx: &[u8], insertion: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(xlsx.to_vec())).unwrap();
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let name: String = file.name().to_string();
        let mut contents: Vec<u8> = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut contents).unwrap();
        if name.starts_with("xl/worksheets/") && name.ends_with(".xml") {
            let text: String = String::from_utf8(contents).unwrap();
            contents = text
                .replace("</worksheet>", &format!("{insertion}</worksheet>"))
                .into_bytes();
        }
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut writer, &contents).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn test_print_options_grid_lines_flags_the_sheet_table() {
    let plain = build_xlsx_bytes("Sheet1", &[("A1", "x"), ("B2", "y")]);
    let flagged = inject_before_worksheet_close(&plain, r#"<printOptions gridLines="1"/>"#);

    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&flagged, &ConvertOptions::default()).unwrap();
    let table = &get_sheet_page(&doc, 0).table;
    assert!(
        table.prints_gridlines,
        "printOptions gridLines must set the table's gridline flag"
    );
    assert!(
        table.paints_borders_inside_boundary,
        "the gridline flag rides on the boundary-band regime"
    );

    let (doc, _warnings) = parser.parse(&plain, &ConvertOptions::default()).unwrap();
    assert!(
        !get_sheet_page(&doc, 0).table.prints_gridlines,
        "a sheet without printOptions must not print gridlines"
    );
}

#[test]
fn test_print_options_headings_prepends_gutter_column_and_letter_strip() {
    let plain = build_xlsx_bytes("Sheet1", &[("A1", "x"), ("B2", "y")]);
    let flagged = inject_before_worksheet_close(&plain, r#"<printOptions headings="1"/>"#);

    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&flagged, &ConvertOptions::default()).unwrap();
    let table = &get_sheet_page(&doc, 0).table;
    assert!(
        table.prints_headings,
        "printOptions headings must set the table's heading flag"
    );
    // GT geometry (issue #623): 23pt gutter track, 13pt letter-strip track.
    assert_eq!(table.column_widths[0], 23.0);
    assert_eq!(table.rows[0].height, Some(13.0));

    // Strip row: empty corner + letters covering the printed columns.
    let strip = &table.rows[0];
    assert!(strip.cells[0].content.is_empty());
    assert_eq!(cell_text(&strip.cells[1]), "A");
    assert_eq!(cell_text(&strip.cells[2]), "B");

    // Gutter cells carry the sheet row numbers; data follows one column later.
    assert_eq!(cell_text(&table.rows[1].cells[0]), "1");
    assert_eq!(cell_text(&table.rows[2].cells[0]), "2");
    assert_eq!(cell_text(&table.rows[1].cells[1]), "x");
    assert_eq!(cell_text(&table.rows[2].cells[2]), "y");

    let (doc, _warnings) = parser.parse(&plain, &ConvertOptions::default()).unwrap();
    let table = &get_sheet_page(&doc, 0).table;
    assert!(
        !table.prints_headings,
        "a sheet without printOptions must not print headings"
    );
    assert_eq!(
        cell_text(&table.rows[0].cells[0]),
        "x",
        "an unflagged sheet must keep its grid unshifted"
    );
}

#[test]
fn test_print_headings_row_numbers_continue_across_manual_page_breaks() {
    // A row break after row 2 splits the sheet; the second segment's gutter
    // must continue at the actual sheet row number, not restart at 1.
    let plain = {
        let mut book = umya_spreadsheet::new_file();
        {
            let sheet = book.get_sheet_mut(&0).unwrap();
            sheet.set_name("Sheet1");
            for (coord, value) in [("A1", "r1"), ("A2", "r2"), ("A3", "r3")] {
                sheet.get_cell_mut(coord).set_value(value);
            }
            let mut brk = umya_spreadsheet::Break::default();
            brk.set_id(2);
            brk.set_manual_page_break(true);
            sheet.get_row_breaks_mut().add_break_list(brk);
        }
        let mut cursor = Cursor::new(Vec::new());
        umya_spreadsheet::writer::xlsx::write_writer(&book, &mut cursor).unwrap();
        cursor.into_inner()
    };
    let flagged = inject_before_worksheet_close(&plain, r#"<printOptions headings="1"/>"#);

    let parser = XlsxParser;
    let (doc, _warnings) = parser.parse(&flagged, &ConvertOptions::default()).unwrap();
    assert_eq!(doc.pages.len(), 2);

    let first = &get_sheet_page(&doc, 0).table;
    assert!(first.prints_headings);
    assert_eq!(cell_text(&first.rows[1].cells[0]), "1");
    assert_eq!(cell_text(&first.rows[2].cells[0]), "2");

    let second = &get_sheet_page(&doc, 1).table;
    assert!(second.prints_headings);
    assert_eq!(cell_text(&second.rows[0].cells[1]), "A");
    assert_eq!(cell_text(&second.rows[1].cells[0]), "3");
    assert_eq!(cell_text(&second.rows[1].cells[1]), "r3");
}
