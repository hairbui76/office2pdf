use super::*;

// ----- US-020: Header/footer parsing tests -----

fn build_docx_with_header(header_text: &str) -> Vec<u8> {
    let header = docx_rs::Header::new().add_paragraph(
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text(header_text)),
    );
    let docx = docx_rs::Docx::new().header(header).add_paragraph(
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Body text")),
    );
    let mut cursor = Cursor::new(Vec::new());
    docx.build().pack(&mut cursor).unwrap();
    cursor.into_inner()
}

fn build_docx_with_footer(footer_text: &str) -> Vec<u8> {
    let footer = docx_rs::Footer::new().add_paragraph(
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text(footer_text)),
    );
    let docx = docx_rs::Docx::new().footer(footer).add_paragraph(
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Body text")),
    );
    let mut cursor = Cursor::new(Vec::new());
    docx.build().pack(&mut cursor).unwrap();
    cursor.into_inner()
}

fn build_docx_with_page_number_footer() -> Vec<u8> {
    let footer = docx_rs::Footer::new().add_paragraph(
        docx_rs::Paragraph::new().add_run(
            docx_rs::Run::new()
                .add_text("Page ")
                .add_field_char(docx_rs::FieldCharType::Begin, false)
                .add_instr_text(docx_rs::InstrText::PAGE(docx_rs::InstrPAGE::new()))
                .add_field_char(docx_rs::FieldCharType::Separate, false)
                .add_text("1")
                .add_field_char(docx_rs::FieldCharType::End, false),
        ),
    );
    let docx = docx_rs::Docx::new().footer(footer).add_paragraph(
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Body text")),
    );
    let mut cursor = Cursor::new(Vec::new());
    docx.build().pack(&mut cursor).unwrap();
    cursor.into_inner()
}

/// Word applies the containing run's properties to the field result, so the
/// parsed field must carry that run's style.
#[test]
fn test_page_number_field_carries_its_run_style() {
    let footer = docx_rs::Footer::new().add_paragraph(
        docx_rs::Paragraph::new().add_run(
            docx_rs::Run::new()
                .size(16)
                .color("888888")
                .add_text("- ")
                .add_field_char(docx_rs::FieldCharType::Begin, false)
                .add_instr_text(docx_rs::InstrText::PAGE(docx_rs::InstrPAGE::new()))
                .add_field_char(docx_rs::FieldCharType::Separate, false)
                .add_field_char(docx_rs::FieldCharType::End, false),
        ),
    );
    let docx = docx_rs::Docx::new().footer(footer).add_paragraph(
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Body text")),
    );
    let mut cursor = Cursor::new(Vec::new());
    docx.build().pack(&mut cursor).unwrap();
    let data = cursor.into_inner();

    let parser = DocxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();
    let flow = match &doc.pages[0] {
        Page::Flow(flow) => flow,
        _ => panic!("Expected FlowPage"),
    };
    let elements = &flow.footer.as_ref().expect("footer").paragraphs[0].elements;
    let style = elements
        .iter()
        .find_map(|element| match element {
            crate::ir::HFInline::PageNumber(style) => Some(style),
            _ => None,
        })
        .expect("page number field parsed");
    assert_eq!(style.font_size, Some(8.0), "w:sz 16 half-points is 8pt");
    assert_eq!(style.color, Some(Color::new(0x88, 0x88, 0x88)));
}

fn build_docx_with_total_pages_footer() -> Vec<u8> {
    let footer = docx_rs::Footer::new().add_paragraph(
        docx_rs::Paragraph::new()
            .add_run(docx_rs::Run::new().add_text("Total "))
            .add_run(
                docx_rs::Run::new()
                    .add_field_char(docx_rs::FieldCharType::Begin, false)
                    .add_instr_text(docx_rs::InstrText::NUMPAGES(docx_rs::InstrNUMPAGES::new()))
                    .add_field_char(docx_rs::FieldCharType::Separate, false)
                    .add_text("1")
                    .add_field_char(docx_rs::FieldCharType::End, false),
            ),
    );
    let docx = docx_rs::Docx::new()
        .footer(footer)
        .add_paragraph(docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Body")));
    let mut cursor = Cursor::new(Vec::new());
    docx.build().pack(&mut cursor).unwrap();
    cursor.into_inner()
}

#[test]
fn test_parse_docx_with_text_header() {
    let data = build_docx_with_header("My Document Header");
    let parser = DocxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let page = match &doc.pages[0] {
        Page::Flow(p) => p,
        _ => panic!("Expected FlowPage"),
    };

    assert!(page.header.is_some(), "FlowPage should have a header");
    let header = page.header.as_ref().unwrap();
    assert!(
        !header.paragraphs.is_empty(),
        "Header should have paragraphs"
    );

    let has_text = header.paragraphs.iter().any(|paragraph| {
        paragraph.elements.iter().any(
            |element| matches!(element, crate::ir::HFInline::Run(run) if run.text.contains("My Document Header")),
        )
    });
    assert!(
        has_text,
        "Header should contain the text 'My Document Header'"
    );
}

#[test]
fn test_parse_docx_with_text_footer() {
    let data = build_docx_with_footer("Footer Text");
    let parser = DocxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let page = match &doc.pages[0] {
        Page::Flow(p) => p,
        _ => panic!("Expected FlowPage"),
    };

    assert!(page.footer.is_some(), "FlowPage should have a footer");
    let footer = page.footer.as_ref().unwrap();

    let has_text = footer.paragraphs.iter().any(|paragraph| {
        paragraph
            .elements
            .iter()
            .any(|element| matches!(element, crate::ir::HFInline::Run(run) if run.text.contains("Footer Text")))
    });
    assert!(has_text, "Footer should contain 'Footer Text'");
}

#[test]
fn test_parse_docx_with_page_number_in_footer() {
    let data = build_docx_with_page_number_footer();
    let parser = DocxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let page = match &doc.pages[0] {
        Page::Flow(p) => p,
        _ => panic!("Expected FlowPage"),
    };

    assert!(page.footer.is_some(), "Should have footer");
    let footer = page.footer.as_ref().unwrap();

    let has_page_num = footer.paragraphs.iter().any(|paragraph| {
        paragraph
            .elements
            .iter()
            .any(|element| matches!(element, crate::ir::HFInline::PageNumber(_)))
    });
    assert!(has_page_num, "Footer should contain a PageNumber field");

    let has_text = footer.paragraphs.iter().any(|paragraph| {
        paragraph
            .elements
            .iter()
            .any(|element| matches!(element, crate::ir::HFInline::Run(run) if run.text.contains("Page ")))
    });
    assert!(
        has_text,
        "Footer should contain 'Page ' text before page number"
    );
}

#[test]
fn test_parse_docx_with_total_pages_in_footer() {
    let data = build_docx_with_total_pages_footer();
    let parser = DocxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let page = match &doc.pages[0] {
        Page::Flow(p) => p,
        _ => panic!("Expected FlowPage"),
    };

    let footer = page.footer.as_ref().expect("Should have footer");
    let has_total_pages = footer.paragraphs.iter().any(|paragraph| {
        paragraph
            .elements
            .iter()
            .any(|element| matches!(element, crate::ir::HFInline::TotalPages(_)))
    });
    assert!(has_total_pages, "Footer should contain a TotalPages field");
}

#[test]
fn test_parse_docx_multiple_sections_with_distinct_page_setup_and_headers() {
    let first_header = docx_rs::Header::new().add_paragraph(
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Section One Header")),
    );
    let second_header = docx_rs::Header::new().add_paragraph(
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Section Two Header")),
    );

    let first_section = docx_rs::Section::new()
        .page_size(docx_rs::PageSize::new().size(12240, 15840))
        .header(first_header)
        .add_paragraph(
            docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Section One")),
        );

    let docx = docx_rs::Docx::new()
        .add_section(first_section)
        .header(second_header)
        .page_size(15840, 12240)
        .add_paragraph(
            docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Section Two")),
        );
    let mut cursor = Cursor::new(Vec::new());
    docx.build().pack(&mut cursor).unwrap();
    let data = cursor.into_inner();

    let parser = DocxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    assert_eq!(doc.pages.len(), 2, "Expected one FlowPage per DOCX section");

    let first_page = match &doc.pages[0] {
        Page::Flow(page) => page,
        _ => panic!("Expected first page to be FlowPage"),
    };
    let second_page = match &doc.pages[1] {
        Page::Flow(page) => page,
        _ => panic!("Expected second page to be FlowPage"),
    };

    assert!(
        (first_page.size.width - 612.0).abs() < 0.1,
        "first page width should come from first section"
    );
    assert!(
        (first_page.size.height - 792.0).abs() < 0.1,
        "first page height should come from first section"
    );
    assert!(
        (second_page.size.width - 792.0).abs() < 0.1,
        "second page width should come from final section"
    );
    assert!(
        (second_page.size.height - 612.0).abs() < 0.1,
        "second page height should come from final section"
    );

    let first_header_text = first_page
        .header
        .as_ref()
        .and_then(|header_footer| {
            header_footer
                .paragraphs
                .iter()
                .flat_map(|paragraph| paragraph.elements.iter())
                .find_map(|element| match element {
                    crate::ir::HFInline::Run(run) => Some(run.text.as_str()),
                    _ => None,
                })
        })
        .unwrap_or("");
    assert_eq!(first_header_text, "Section One Header");

    let second_header_text = second_page
        .header
        .as_ref()
        .and_then(|header_footer| {
            header_footer
                .paragraphs
                .iter()
                .flat_map(|paragraph| paragraph.elements.iter())
                .find_map(|element| match element {
                    crate::ir::HFInline::Run(run) => Some(run.text.as_str()),
                    _ => None,
                })
        })
        .unwrap_or("");
    assert_eq!(second_header_text, "Section Two Header");
}

#[test]
fn test_parse_docx_with_header_and_footer() {
    let header = docx_rs::Header::new().add_paragraph(
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Header Text")),
    );
    let footer = docx_rs::Footer::new().add_paragraph(
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Footer Text")),
    );
    let docx = docx_rs::Docx::new()
        .header(header)
        .footer(footer)
        .add_paragraph(docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Body")));
    let mut cursor = Cursor::new(Vec::new());
    docx.build().pack(&mut cursor).unwrap();
    let data = cursor.into_inner();

    let parser = DocxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let page = match &doc.pages[0] {
        Page::Flow(p) => p,
        _ => panic!("Expected FlowPage"),
    };

    assert!(page.header.is_some(), "Should have header");
    assert!(page.footer.is_some(), "Should have footer");
}

#[test]
fn test_parse_docx_without_header_footer() {
    let data = build_docx_bytes(vec![
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Just text")),
    ]);
    let parser = DocxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let page = match &doc.pages[0] {
        Page::Flow(p) => p,
        _ => panic!("Expected FlowPage"),
    };

    assert!(page.header.is_none(), "No header expected");
    assert!(page.footer.is_none(), "No footer expected");
}

// ----- Page orientation tests -----

#[test]
fn test_portrait_document_width_less_than_height() {
    let data = build_docx_bytes_with_page_setup(
        vec![docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Portrait"))],
        11906,
        16838,
        1440,
        1440,
        1440,
        1440,
    );
    let parser = DocxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let page = match &doc.pages[0] {
        Page::Flow(p) => p,
        _ => panic!("Expected FlowPage"),
    };
    assert!(
        page.size.width < page.size.height,
        "Portrait: width ({}) should be < height ({})",
        page.size.width,
        page.size.height
    );
}

#[test]
fn test_landscape_document_width_greater_than_height() {
    let data = build_docx_bytes_with_page_setup(
        vec![docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Landscape"))],
        16838,
        11906,
        1440,
        1440,
        1440,
        1440,
    );
    let parser = DocxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let page = match &doc.pages[0] {
        Page::Flow(p) => p,
        _ => panic!("Expected FlowPage"),
    };
    assert!(
        page.size.width > page.size.height,
        "Landscape: width ({}) should be > height ({})",
        page.size.width,
        page.size.height
    );
    assert!(
        (page.size.width - 841.9).abs() < 1.0,
        "Expected width ~841.9, got {}",
        page.size.width
    );
    assert!(
        (page.size.height - 595.3).abs() < 1.0,
        "Expected height ~595.3, got {}",
        page.size.height
    );
}

#[test]
fn test_default_document_is_portrait() {
    let data = build_docx_bytes(vec![
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Default")),
    ]);
    let parser = DocxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let page = match &doc.pages[0] {
        Page::Flow(p) => p,
        _ => panic!("Expected FlowPage"),
    };
    assert!(
        page.size.width < page.size.height,
        "Default should be portrait: width ({}) < height ({})",
        page.size.width,
        page.size.height
    );
}

#[test]
fn test_landscape_with_orient_attribute() {
    let mut docx = docx_rs::Docx::new()
        .page_size(16838, 11906)
        .page_orient(docx_rs::PageOrientationType::Landscape)
        .page_margin(
            docx_rs::PageMargin::new()
                .top(1440)
                .bottom(1440)
                .left(1440)
                .right(1440),
        );
    docx = docx.add_paragraph(
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Landscape with orient")),
    );
    let buf = Vec::new();
    let mut cursor = Cursor::new(buf);
    docx.build().pack(&mut cursor).unwrap();
    let data = cursor.into_inner();

    let parser = DocxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();

    let page = match &doc.pages[0] {
        Page::Flow(p) => p,
        _ => panic!("Expected FlowPage"),
    };
    assert!(
        page.size.width > page.size.height,
        "Landscape with orient: width ({}) should be > height ({})",
        page.size.width,
        page.size.height
    );
}

#[test]
fn test_extract_page_size_orient_landscape_swaps_dimensions() {
    let page_size = docx_rs::PageSize::new()
        .width(11906)
        .height(16838)
        .orient(docx_rs::PageOrientationType::Landscape);

    let result = extract_page_size(&page_size);
    assert!(
        result.width > result.height,
        "orient=landscape should ensure width ({}) > height ({})",
        result.width,
        result.height
    );
}

#[test]
fn test_extract_page_size_no_orient_keeps_dimensions() {
    let page_size = docx_rs::PageSize::new().width(11906).height(16838);

    let result = extract_page_size(&page_size);
    assert!(
        result.width < result.height,
        "No orient: width ({}) should be < height ({})",
        result.width,
        result.height
    );
}

/// Word letterhead headers declare the gap between text and rule with
/// `w:pBdr/<side>/@w:space`, in points.
#[test]
fn test_parse_docx_header_paragraph_border_space() {
    let mut paragraph =
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Manual v0.6"));
    paragraph.property = paragraph.property.set_borders(
        docx_rs::ParagraphBorders::with_empty().set(
            docx_rs::ParagraphBorder::new(docx_rs::ParagraphBorderPosition::Bottom)
                .val(docx_rs::BorderType::Single)
                .size(4)
                .space(4)
                .color("CCCCCC"),
        ),
    );
    let header = docx_rs::Header::new().add_paragraph(paragraph);
    let docx = docx_rs::Docx::new().header(header).add_paragraph(
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Body text")),
    );
    let mut cursor = Cursor::new(Vec::new());
    docx.build().pack(&mut cursor).unwrap();
    let data = cursor.into_inner();

    let parser = DocxParser;
    let (doc, _warnings) = parser.parse(&data, &ConvertOptions::default()).unwrap();
    let flow = match &doc.pages[0] {
        Page::Flow(flow) => flow,
        _ => panic!("Expected FlowPage"),
    };
    let paragraph = &flow.header.as_ref().expect("header").paragraphs[0];

    let border = paragraph.border.as_ref().expect("bottom rule parsed");
    assert!(border.bottom.is_some());
    let space = paragraph.border_space.expect("w:space parsed");
    assert_eq!(space.bottom, 4.0);
    assert_eq!(space.top, 0.0);
}

// ----- Document grid (`w:docGrid`) parsing tests (issue #518) -----

/// A one-paragraph document whose section carries `<w:docGrid w:linePitch="360"
/// {type_attribute}>`, written as raw XML because docx-rs's builder cannot
/// place a `w:docGrid` on the body section.
fn build_docx_with_doc_grid(type_attribute: &str) -> Vec<u8> {
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default();

    zip.start_file("[Content_Types].xml", opts).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    )
    .unwrap();

    zip.start_file("_rels/.rels", opts).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("word/document.xml", opts).unwrap();
    let document_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t xml:space="preserve">본문 한 줄</w:t></w:r></w:p>
    <w:sectPr>
      <w:pgSz w:w="11906" w:h="16838"/>
      <w:docGrid w:linePitch="360"{type_attribute}/>
    </w:sectPr>
  </w:body>
</w:document>"#
    );
    zip.write_all(document_xml.as_bytes()).unwrap();

    zip.finish().unwrap().into_inner()
}

fn parse_flow_page(data: &[u8]) -> FlowPage {
    let (doc, _warnings) = DocxParser
        .parse(data, &ConvertOptions::default())
        .expect("document parses");
    match &doc.pages[0] {
        Page::Flow(flow) => flow.clone(),
        _ => panic!("Expected FlowPage"),
    }
}

#[test]
fn doc_grid_without_a_type_declares_a_pitch_that_does_not_snap() {
    // Word writes a bare `<w:docGrid w:linePitch="360"/>` into ordinary Korean
    // documents. `w:type` then takes its default value `default`, which is
    // ECMA-376's name for *no* grid, and Word lays the file out with none —
    // every Korean fixture in the business corpus is like this and none of
    // their line advances is a multiple of 18pt (issue #518).
    let page = parse_flow_page(&build_docx_with_doc_grid(""));

    assert_eq!(
        page.line_grid_pitch,
        Some(18.0),
        "the declared pitch is still read: it marks an East Asian edition"
    );
    assert!(
        !page.line_grid_snaps_lines,
        "a `default` grid must not snap lines to that pitch"
    );
}

#[test]
fn doc_grid_typed_lines_snaps_lines_to_the_pitch() {
    // Triangulation: the author turning the grid on is what makes it real.
    for grid_type in ["lines", "linesAndChars", "snapToChars"] {
        let page = parse_flow_page(&build_docx_with_doc_grid(&format!(
            r#" w:type="{grid_type}""#
        )));

        assert_eq!(page.line_grid_pitch, Some(18.0));
        assert!(
            page.line_grid_snaps_lines,
            "w:type=\"{grid_type}\" snaps lines to the grid"
        );
    }
}

#[test]
fn a_section_without_a_doc_grid_has_no_pitch_at_all() {
    let docx = docx_rs::Docx::new()
        .add_paragraph(docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Body")));
    let mut cursor = Cursor::new(Vec::new());
    docx.build().pack(&mut cursor).unwrap();

    let page = parse_flow_page(&cursor.into_inner());

    assert_eq!(page.line_grid_pitch, None);
    assert!(!page.line_grid_snaps_lines);
}

// ----- Body paragraph `w:pBdr w:space` parsing (issue #520) -----

/// A one-paragraph document whose only paragraph carries a bottom rule with
/// the given `w:space`, in points.
fn build_docx_with_paragraph_rule(space: Option<usize>) -> Vec<u8> {
    let mut border = docx_rs::ParagraphBorder::new(docx_rs::ParagraphBorderPosition::Bottom)
        .val(docx_rs::BorderType::Double)
        .size(8)
        .color("000000");
    if let Some(space) = space {
        border = border.space(space);
    }
    let mut paragraph =
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Letterhead"));
    paragraph.property = paragraph
        .property
        .set_borders(docx_rs::ParagraphBorders::with_empty().set(border));

    let docx = docx_rs::Docx::new().add_paragraph(paragraph);
    let mut cursor = Cursor::new(Vec::new());
    docx.build().pack(&mut cursor).unwrap();
    cursor.into_inner()
}

fn first_body_paragraph_style(data: &[u8]) -> ParagraphStyle {
    let (doc, _warnings) = DocxParser
        .parse(data, &ConvertOptions::default())
        .expect("document parses");
    let flow = match &doc.pages[0] {
        Page::Flow(flow) => flow,
        _ => panic!("Expected FlowPage"),
    };
    match &flow.content[0] {
        Block::Paragraph(paragraph) => paragraph.style.clone(),
        other => panic!("Expected a paragraph, got {other:?}"),
    }
}

#[test]
fn body_paragraph_rule_carries_its_declared_space() {
    // The gap between a paragraph's text and its rule is the paragraph's own
    // `w:space`, in points. Substituting a fixed 4pt displaced everything
    // below a bordered paragraph by the difference (issue #520).
    let style = first_body_paragraph_style(&build_docx_with_paragraph_rule(Some(8)));

    let space = style.border_space.expect("w:space parsed");
    assert_eq!(space.bottom, 8.0);
    assert_eq!((space.top, space.left, space.right), (0.0, 0.0, 0.0));
}

#[test]
fn a_rule_without_w_space_yields_no_gap() {
    // Triangulation: the attribute's own default is 0, so an omitted `w:space`
    // must not resurrect a house value.
    let style = first_body_paragraph_style(&build_docx_with_paragraph_rule(None));

    assert!(style.border.is_some(), "the rule itself is still parsed");
    assert_eq!(style.border_space.map(|space| space.bottom), Some(0.0));
}

#[test]
fn a_paragraph_without_a_rule_has_no_border_space() {
    let docx = docx_rs::Docx::new()
        .add_paragraph(docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Body")));
    let mut cursor = Cursor::new(Vec::new());
    docx.build().pack(&mut cursor).unwrap();

    let style = first_body_paragraph_style(&cursor.into_inner());

    assert!(style.border.is_none());
    assert!(style.border_space.is_none());
}

// ----- Word's East Asian/Latin auto space (issue #521) -----

/// The in-text marker the parser places at such a boundary. Duplicated from
/// the parser so the test pins the wire format rather than the constant.
const AUTO_SPACE_MARKER: char = '\u{E001}';

/// A one-paragraph document whose only run holds `text`, optionally justified.
fn build_docx_with_korean_text(text: &str, justified: bool) -> Vec<u8> {
    let mut paragraph = docx_rs::Paragraph::new().add_run(
        docx_rs::Run::new()
            .add_text(text)
            .fonts(docx_rs::RunFonts::new().east_asia("Malgun Gothic"))
            .size(21),
    );
    if justified {
        paragraph = paragraph.align(docx_rs::AlignmentType::Both);
    }
    let docx = docx_rs::Docx::new().add_paragraph(paragraph);
    let mut cursor = Cursor::new(Vec::new());
    docx.build().pack(&mut cursor).unwrap();
    cursor.into_inner()
}

fn first_paragraph_text(data: &[u8]) -> String {
    let (doc, _warnings) = DocxParser
        .parse(data, &ConvertOptions::default())
        .expect("document parses");
    let flow = match &doc.pages[0] {
        Page::Flow(flow) => flow,
        _ => panic!("Expected FlowPage"),
    };
    match &flow.content[0] {
        Block::Paragraph(paragraph) => paragraph.runs.iter().map(|run| run.text.as_str()).collect(),
        other => panic!("Expected a paragraph, got {other:?}"),
    }
}

#[test]
fn a_boundary_between_east_asian_text_and_a_number_carries_the_auto_space() {
    // Word inserts a quarter em where East Asian text meets a Latin letter or
    // digit with no literal space between, on both sides of the island. A
    // native export measures 2.625pt at 10.5pt and 2.375pt at 9.5pt, and our
    // output was that much narrower at every such boundary (issue #521).
    let text = first_paragraph_text(&build_docx_with_korean_text("2026년 제3자", false));

    assert_eq!(
        text,
        format!("2026{AUTO_SPACE_MARKER}년 제{AUTO_SPACE_MARKER}3{AUTO_SPACE_MARKER}자"),
        "both sides of a digit island widen, and only boundaries without a \
         literal space do"
    );
}

#[test]
fn a_boundary_that_already_has_a_space_gets_nothing() {
    // Triangulation: Word adds nothing where the author already typed a space,
    // which is why `은 2026` measures the same in the GT as in our output.
    let text = first_paragraph_text(&build_docx_with_korean_text("유효기간은 2026", false));

    assert!(
        !text.contains(AUTO_SPACE_MARKER),
        "a literal space already separates the two scripts: {text:?}"
    );
}

#[test]
fn a_justified_paragraph_keeps_its_lines_as_authored() {
    // Word treats the space as compressible and justification absorbs it:
    // every boundary that lacks it in the corpus GT is on a line Word is
    // actively stretching or compressing. Adding a rigid one there re-wrapped
    // the line instead (issue #521).
    let text = first_paragraph_text(&build_docx_with_korean_text("2026년 제3자", true));

    assert!(
        !text.contains(AUTO_SPACE_MARKER),
        "a justified paragraph absorbs the space rather than showing it: {text:?}"
    );
}

#[test]
fn latin_only_and_east_asian_only_text_are_untouched() {
    // Triangulation on both sides of the predicate: the rule needs one of each
    // script, so neither a pure Latin run nor a pure Korean one may widen.
    for text in ["Version 2026 release 3", "계약서를 작성하여 보관한다"] {
        let parsed = first_paragraph_text(&build_docx_with_korean_text(text, false));
        assert!(
            !parsed.contains(AUTO_SPACE_MARKER),
            "single-script text needs no auto space: {parsed:?}"
        );
    }
}

#[test]
fn cjk_punctuation_is_not_a_boundary() {
    // `is_east_asian_text` is deliberately narrower than the renderer's
    // `is_cjk_like`: CJK punctuation and the fullwidth forms are already
    // full-width, and Word adds nothing beside them.
    let text = first_paragraph_text(&build_docx_with_korean_text("、2026", false));

    assert!(
        !text.contains(AUTO_SPACE_MARKER),
        "an ideographic comma is already full-width: {text:?}"
    );
}
