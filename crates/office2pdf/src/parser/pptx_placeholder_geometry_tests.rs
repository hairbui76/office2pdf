use super::*;
use std::io::Write;
use zip::write::SimpleFileOptions;

// ── Helpers ──────────────────────────────────────────────────────────

const LAYOUT_HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8"?><p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"#;
const LAYOUT_FOOTER: &str = "</p:spTree></p:cSld></p:sldLayout>";

const MASTER_HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8"?><p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"#;
const MASTER_FOOTER: &str = "</p:spTree></p:cSld></p:sldMaster>";

/// A placeholder `<p:sp>` for a slide, layout, or master.
/// `ph_attrs` is the raw attribute string of `<p:ph>` (e.g. `type="title"` or `idx="1"`).
/// `xfrm_emu` is `Some((x, y, cx, cy))` for an explicit `<a:xfrm>`, or `None` to inherit.
fn make_placeholder_sp(
    ph_attrs: &str,
    xfrm_emu: Option<(i64, i64, i64, i64)>,
    text: &str,
) -> String {
    let sp_pr: String = match xfrm_emu {
        Some((x, y, cx, cy)) => format!(
            r#"<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm></p:spPr>"#
        ),
        None => "<p:spPr/>".to_string(),
    };
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Placeholder"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph {ph_attrs}/></p:nvPr></p:nvSpPr>{sp_pr}<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp>"#
    )
}

fn make_slide_with_shapes(shapes: &[String]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"#,
    );
    for shape in shapes {
        xml.push_str(shape);
    }
    xml.push_str("</p:spTree></p:cSld></p:sld>");
    xml
}

fn make_layout_with_shapes(shapes: &[String]) -> String {
    let mut xml = String::from(LAYOUT_HEADER);
    for shape in shapes {
        xml.push_str(shape);
    }
    xml.push_str(LAYOUT_FOOTER);
    xml
}

fn make_master_with_shapes(shapes: &[String]) -> String {
    let mut xml = String::from(MASTER_HEADER);
    for shape in shapes {
        xml.push_str(shape);
    }
    xml.push_str(MASTER_FOOTER);
    xml
}

fn parse_document(data: &[u8]) -> Document {
    let parser = PptxParser;
    let (doc, _warnings) = parser.parse(data, &ConvertOptions::default()).unwrap();
    doc
}

/// Find the text box element containing the given text.
fn find_text_box_with_text<'a>(page: &'a FixedPage, needle: &str) -> &'a FixedElement {
    page.elements
        .iter()
        .find(|element| {
            if let FixedElementKind::TextBox(text_box) = &element.kind {
                text_box.content.iter().any(|block| match block {
                    Block::Paragraph(paragraph) => {
                        paragraph.runs.iter().any(|run| run.text.contains(needle))
                    }
                    _ => false,
                })
            } else {
                false
            }
        })
        .unwrap_or_else(|| panic!("no text box containing {needle:?}"))
}

fn assert_geometry(element: &FixedElement, x_emu: i64, y_emu: i64, cx_emu: i64, cy_emu: i64) {
    let expected: [f64; 4] = [
        emu_to_pt(x_emu),
        emu_to_pt(y_emu),
        emu_to_pt(cx_emu),
        emu_to_pt(cy_emu),
    ];
    let actual: [f64; 4] = [element.x, element.y, element.width, element.height];
    for (index, (value, want)) in actual.iter().zip(expected.iter()).enumerate() {
        assert!(
            (value - want).abs() < 0.01,
            "geometry component {index}: got {value}, want {want} (element: x={} y={} w={} h={})",
            element.x,
            element.y,
            element.width,
            element.height
        );
    }
}

// ── Slide → layout inheritance ───────────────────────────────────────

#[test]
fn test_title_placeholder_inherits_layout_geometry() {
    let slide = make_slide_with_shapes(&[make_placeholder_sp(r#"type="title""#, None, "Hello")]);
    let layout = make_layout_with_shapes(&[make_placeholder_sp(
        r#"type="title""#,
        Some((457_200, 274_638, 8_229_600, 1_143_000)),
        "Layout title",
    )]);
    let master = make_master_with_shapes(&[]);
    let data = build_test_pptx_with_layout_master(SLIDE_CX, SLIDE_CY, &slide, &layout, &master);

    let doc = parse_document(&data);
    let page = first_fixed_page(&doc);
    let element = find_text_box_with_text(page, "Hello");
    assert_geometry(element, 457_200, 274_638, 8_229_600, 1_143_000);
}

#[test]
fn test_ctr_title_placeholder_matches_layout_title_family() {
    // A slide `title` placeholder must match a layout `ctrTitle` placeholder
    // (and vice versa): both belong to the title family.
    let slide = make_slide_with_shapes(&[make_placeholder_sp(r#"type="title""#, None, "Hello")]);
    let layout = make_layout_with_shapes(&[make_placeholder_sp(
        r#"type="ctrTitle""#,
        Some((685_800, 2_130_425, 7_772_400, 1_470_025)),
        "Centered layout title",
    )]);
    let master = make_master_with_shapes(&[]);
    let data = build_test_pptx_with_layout_master(SLIDE_CX, SLIDE_CY, &slide, &layout, &master);

    let doc = parse_document(&data);
    let page = first_fixed_page(&doc);
    let element = find_text_box_with_text(page, "Hello");
    assert_geometry(element, 685_800, 2_130_425, 7_772_400, 1_470_025);
}

#[test]
fn test_body_placeholders_match_layout_by_idx() {
    let slide = make_slide_with_shapes(&[
        make_placeholder_sp(r#"type="body" idx="1""#, None, "Left"),
        make_placeholder_sp(r#"type="body" idx="2""#, None, "Right"),
    ]);
    let layout = make_layout_with_shapes(&[
        make_placeholder_sp(
            r#"type="body" idx="1""#,
            Some((457_200, 1_600_200, 4_038_600, 4_525_963)),
            "Layout left",
        ),
        make_placeholder_sp(
            r#"type="body" idx="2""#,
            Some((4_648_200, 1_600_200, 4_038_600, 4_525_963)),
            "Layout right",
        ),
    ]);
    let master = make_master_with_shapes(&[]);
    let data = build_test_pptx_with_layout_master(SLIDE_CX, SLIDE_CY, &slide, &layout, &master);

    let doc = parse_document(&data);
    let page = first_fixed_page(&doc);
    let left = find_text_box_with_text(page, "Left");
    assert_geometry(left, 457_200, 1_600_200, 4_038_600, 4_525_963);
    let right = find_text_box_with_text(page, "Right");
    assert_geometry(right, 4_648_200, 1_600_200, 4_038_600, 4_525_963);
}

// ── Layout → master fallback ─────────────────────────────────────────

#[test]
fn test_layout_placeholder_without_geometry_falls_back_to_master() {
    // The layout declares the placeholder but omits <a:xfrm>; geometry must
    // come from the master's matching placeholder.
    let slide =
        make_slide_with_shapes(&[make_placeholder_sp(r#"type="body" idx="1""#, None, "Hello")]);
    let layout = make_layout_with_shapes(&[make_placeholder_sp(
        r#"type="body" idx="1""#,
        None,
        "Layout body",
    )]);
    let master = make_master_with_shapes(&[make_placeholder_sp(
        r#"type="body" idx="1""#,
        Some((457_200, 1_600_200, 8_229_600, 4_525_963)),
        "Master body",
    )]);
    let data = build_test_pptx_with_layout_master(SLIDE_CX, SLIDE_CY, &slide, &layout, &master);

    let doc = parse_document(&data);
    let page = first_fixed_page(&doc);
    let element = find_text_box_with_text(page, "Hello");
    assert_geometry(element, 457_200, 1_600_200, 8_229_600, 4_525_963);
}

#[test]
fn test_subtitle_placeholder_falls_back_to_master_body() {
    // `subTitle` has no direct master counterpart; it must normalize to the
    // master `body` placeholder when the layout provides no geometry.
    let slide = make_slide_with_shapes(&[make_placeholder_sp(
        r#"type="subTitle" idx="1""#,
        None,
        "Hello",
    )]);
    let layout = make_layout_with_shapes(&[]);
    let master = make_master_with_shapes(&[make_placeholder_sp(
        r#"type="body" idx="1""#,
        Some((1_371_600, 3_886_200, 6_400_800, 1_752_600)),
        "Master body",
    )]);
    let data = build_test_pptx_with_layout_master(SLIDE_CX, SLIDE_CY, &slide, &layout, &master);

    let doc = parse_document(&data);
    let page = first_fixed_page(&doc);
    let element = find_text_box_with_text(page, "Hello");
    assert_geometry(element, 1_371_600, 3_886_200, 6_400_800, 1_752_600);
}

#[test]
fn test_footer_placeholder_matches_master_by_type_despite_idx_mismatch() {
    // Real decks give the footer different idx values on each level
    // (layout idx="11", master idx="4"); the footer family matches by type.
    let slide = make_slide_with_shapes(&[make_placeholder_sp(
        r#"type="ftr" sz="quarter" idx="11""#,
        None,
        "Prislista",
    )]);
    let layout = make_layout_with_shapes(&[make_placeholder_sp(
        r#"type="ftr" sz="quarter" idx="11""#,
        None,
        "Layout footer",
    )]);
    let master = make_master_with_shapes(&[make_placeholder_sp(
        r#"type="ftr" sz="quarter" idx="4""#,
        Some((3_124_200, 6_356_350, 2_895_600, 365_125)),
        "Master footer",
    )]);
    let data = build_test_pptx_with_layout_master(SLIDE_CX, SLIDE_CY, &slide, &layout, &master);

    let doc = parse_document(&data);
    let page = first_fixed_page(&doc);
    let element = find_text_box_with_text(page, "Prislista");
    assert_geometry(element, 3_124_200, 6_356_350, 2_895_600, 365_125);
}

// ── Explicit slide geometry wins ─────────────────────────────────────

#[test]
fn test_placeholder_with_explicit_xfrm_keeps_own_geometry() {
    let slide = make_slide_with_shapes(&[make_placeholder_sp(
        r#"type="title""#,
        Some((914_400, 914_400, 3_657_600, 914_400)),
        "Hello",
    )]);
    let layout = make_layout_with_shapes(&[make_placeholder_sp(
        r#"type="title""#,
        Some((457_200, 274_638, 8_229_600, 1_143_000)),
        "Layout title",
    )]);
    let master = make_master_with_shapes(&[]);
    let data = build_test_pptx_with_layout_master(SLIDE_CX, SLIDE_CY, &slide, &layout, &master);

    let doc = parse_document(&data);
    let page = first_fixed_page(&doc);
    let element = find_text_box_with_text(page, "Hello");
    assert_geometry(element, 914_400, 914_400, 3_657_600, 914_400);
}

// ── Picture placeholder ──────────────────────────────────────────────

/// Build a PPTX with one slide (with an image), one layout, and one master.
fn build_test_pptx_with_layout_master_and_image(
    slide_xml: &str,
    layout_xml: &str,
    master_xml: &str,
    image_bytes: &[u8],
) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default();

    let ct = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="bmp" ContentType="image/bmp"/><Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/></Types>"#;
    zip.start_file("[Content_Types].xml", opts).unwrap();
    zip.write_all(ct.as_bytes()).unwrap();

    zip.start_file("_rels/.rels", opts).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#,
    )
    .unwrap();

    let pres = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldSz cx="{SLIDE_CX}" cy="{SLIDE_CY}"/><p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst></p:presentation>"#,
    );
    zip.start_file("ppt/presentation.xml", opts).unwrap();
    zip.write_all(pres.as_bytes()).unwrap();

    zip.start_file("ppt/_rels/presentation.xml.rels", opts)
        .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/></Relationships>"#,
    )
    .unwrap();

    zip.start_file("ppt/slides/slide1.xml", opts).unwrap();
    zip.write_all(slide_xml.as_bytes()).unwrap();

    zip.start_file("ppt/slides/_rels/slide1.xml.rels", opts)
        .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId10" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.bmp"/></Relationships>"#,
    )
    .unwrap();

    zip.start_file("ppt/slideLayouts/slideLayout1.xml", opts)
        .unwrap();
    zip.write_all(layout_xml.as_bytes()).unwrap();

    zip.start_file("ppt/slideLayouts/_rels/slideLayout1.xml.rels", opts)
        .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#,
    )
    .unwrap();

    zip.start_file("ppt/slideMasters/slideMaster1.xml", opts)
        .unwrap();
    zip.write_all(master_xml.as_bytes()).unwrap();

    zip.start_file("ppt/media/image1.bmp", opts).unwrap();
    zip.write_all(image_bytes).unwrap();

    zip.finish().unwrap().into_inner()
}

#[test]
fn test_picture_placeholder_inherits_layout_geometry() {
    let pic = r#"<p:pic><p:nvPicPr><p:cNvPr id="3" name="Picture"/><p:cNvPicPr/><p:nvPr><p:ph type="pic" idx="1"/></p:nvPr></p:nvPicPr><p:blipFill><a:blip r:embed="rId10"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr/></p:pic>"#;
    let slide = make_slide_with_shapes(&[pic.to_string()]);
    let layout = make_layout_with_shapes(&[make_placeholder_sp(
        r#"type="pic" idx="1""#,
        Some((2_286_000, 1_143_000, 4_572_000, 3_429_000)),
        "Layout picture caption",
    )]);
    let master = make_master_with_shapes(&[]);
    let data = build_test_pptx_with_layout_master_and_image(
        &slide,
        &layout,
        &master,
        &image_tests::make_test_bmp(),
    );

    let doc = parse_document(&data);
    let page = first_fixed_page(&doc);
    let element = page
        .elements
        .iter()
        .find(|element| matches!(element.kind, FixedElementKind::Image(_)))
        .expect("no image element on page");
    assert_geometry(element, 2_286_000, 1_143_000, 4_572_000, 3_429_000);
}
