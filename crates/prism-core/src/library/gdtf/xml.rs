//! Just enough of an XML tree to read a `description.xml` — **S60**.
//!
//! `quick-xml` is a pull parser, and a pull parser is the right thing to read a
//! megabyte of XML with. What it is not is a convenient thing to ask a dozen
//! unrelated questions of: the geometry tree, the attribute table, the wheels
//! and the DMX modes are four walks over four parts of one document, and doing
//! them in one pass would be one state machine holding four jobs.
//!
//! So the document is read **once** into this, and the reader above walks it as
//! a tree. A `description.xml` of the largest fixture published is a couple of
//! megabytes; [`MAX_DOCUMENT`] is what stops a hand-built file being larger.
//!
//! Nothing here fails. A document that is not XML, or that nests deeper than
//! [`MAX_DEPTH`], answers [`None`] and the file is counted as rejected.

use quick_xml::events::Event;

/// The most XML a `description.xml` may be — 32 MiB.
///
/// Read from an archive entry that is already capped at
/// [`super::super::zip::MAX_FILE`]; this is the second, smaller cap, because a
/// 64 MiB XML document is not a fixture description.
pub const MAX_DOCUMENT: usize = 32 * 1024 * 1024;

/// How deep the element tree may nest.
///
/// A geometry tree of a moving head is four deep and the deepest path in the
/// format — `GDTF/FixtureType/DMXModes/DMXMode/DMXChannels/DMXChannel/
/// LogicalChannel/ChannelFunction/ChannelSet` — is nine. Sixty-four is far past
/// anything the format produces and stops a file whose elements are never
/// closed from recursing this reader's caller.
pub const MAX_DEPTH: usize = 64;

/// One element: its name, its attributes and its children.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Node {
    /// The element's name, without a namespace prefix.
    pub name: String,
    /// Its attributes, in the order they were written.
    pub attributes: Vec<(String, String)>,
    /// Its child elements. Text content is not kept: GDTF states everything in
    /// attributes and this reader has no use for the one exception.
    pub children: Vec<Node>,
}

impl Node {
    /// One attribute's value, or `None` when the element does not carry it.
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// One attribute's value, or `""`.
    ///
    /// What most of the reader wants: GDTF's defaults are almost all *the empty
    /// string means it was not stated*, and an `Option` at every call site would
    /// say nothing the empty string does not.
    #[must_use]
    pub fn get(&self, name: &str) -> &str {
        self.attribute(name).unwrap_or_default()
    }

    /// The first child called `name`.
    #[must_use]
    pub fn child(&self, name: &str) -> Option<&Self> {
        self.children.iter().find(|child| child.name == name)
    }

    /// The child at the end of a path of element names.
    ///
    /// `node.path(&["AttributeDefinitions", "Attributes"])` is the attribute
    /// table, and `None` when the file states neither.
    #[must_use]
    pub fn path(&self, names: &[&str]) -> Option<&Self> {
        let mut at = self;
        for name in names {
            at = at.child(name)?;
        }
        Some(at)
    }

    /// Every child called `name`, in order.
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Self> {
        self.children.iter().filter(move |child| child.name == name)
    }
}

/// Reads a whole document into one [`Node`], which is its root element.
///
/// Answers `None` for anything that is not XML, for a document larger than
/// [`MAX_DOCUMENT`], and for one that nests past [`MAX_DEPTH`].
#[must_use]
pub fn parse(source: &[u8]) -> Option<Node> {
    if source.len() > MAX_DOCUMENT {
        return None;
    }
    let mut reader = quick_xml::Reader::from_reader(source);
    let config = reader.config_mut();
    // A description written by a tool that closes its own tags is the ordinary
    // case; one that does not is a file this desk refuses rather than guesses
    // at.
    config.check_end_names = true;
    config.trim_text(true);

    // The open elements, root last. A document with several roots keeps the
    // first, which is what every XML reader does with one.
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) => {
                if stack.len() >= MAX_DEPTH {
                    return None;
                }
                stack.push(element(&start)?);
            }
            Ok(Event::Empty(empty)) => {
                let node = element(&empty)?;
                close(&mut stack, &mut root, node);
            }
            Ok(Event::End(_)) => {
                let node = stack.pop()?;
                close(&mut stack, &mut root, node);
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return None,
        }
        buffer.clear();
    }
    // Elements left open: the document was truncated.
    if stack.is_empty() { root } else { None }
}

/// Files a finished element under its parent, or keeps it as the root.
///
/// A document with several roots keeps the first, which is what every XML
/// reader does with one.
fn close(stack: &mut [Node], root: &mut Option<Node>, node: Node) {
    match stack.last_mut() {
        Some(parent) => parent.children.push(node),
        None => {
            root.get_or_insert(node);
        }
    }
}

/// One element's name and attributes.
///
/// A namespace prefix is dropped — GDTF states none, and a document that
/// carried one would otherwise have every element named `gdtf:Something` and
/// match nothing.
fn element(tag: &quick_xml::events::BytesStart<'_>) -> Option<Node> {
    let raw = tag.name();
    let name = std::str::from_utf8(raw.as_ref()).ok()?;
    let name = name.rsplit(':').next().unwrap_or(name).to_owned();
    let mut attributes = Vec::new();
    for attribute in tag.attributes() {
        let Ok(attribute) = attribute else {
            return None;
        };
        let key = std::str::from_utf8(attribute.key.as_ref()).ok()?;
        let key = key.rsplit(':').next().unwrap_or(key).to_owned();
        // Entities and character references resolved, because a fixture called
        // *Mac&#38;nbsp;Aura* is a fixture called *Mac Aura*. GDTF states
        // version 1.0, and a `description.xml` carrying a 1.1 declaration is a
        // file nothing in this format produces.
        let Ok(value) = attribute.normalized_value(quick_xml::XmlVersion::Implicit1_0) else {
            return None;
        };
        attributes.push((key, value.into_owned()));
    }
    Some(Node {
        name,
        attributes,
        children: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::{MAX_DEPTH, parse};

    #[test]
    fn an_element_keeps_its_attributes_and_its_children() {
        let root = parse(
            br#"<GDTF DataVersion="1.2">
                  <FixtureType Name="T1" Manufacturer="Robe">
                    <Wheels><Wheel Name="Gobo1"/></Wheels>
                  </FixtureType>
                </GDTF>"#,
        )
        .expect("it is XML");
        assert_eq!(root.name, "GDTF");
        assert_eq!(root.get("DataVersion"), "1.2");
        let fixture = root.child("FixtureType").expect("there is one");
        assert_eq!(fixture.get("Name"), "T1");
        assert_eq!(fixture.get("Manufacturer"), "Robe");
        assert_eq!(
            fixture
                .path(&["Wheels", "Wheel"])
                .expect("the wheel is there")
                .get("Name"),
            "Gobo1"
        );
    }

    #[test]
    fn an_empty_element_is_a_child_like_any_other() {
        let root = parse(b"<a><b x=\"1\"/><b x=\"2\"/></a>").expect("it is XML");
        let seen: Vec<&str> = root.children_named("b").map(|node| node.get("x")).collect();
        assert_eq!(seen, ["1", "2"]);
    }

    #[test]
    fn an_entity_is_resolved() {
        let root = parse(b"<a n=\"Mac &amp; Aura\"/>").expect("it is XML");
        assert_eq!(root.get("n"), "Mac & Aura");
    }

    #[test]
    fn a_namespace_prefix_is_dropped() {
        let root = parse(b"<g:GDTF xmlns:g=\"urn:x\"><g:FixtureType g:Name=\"T\"/></g:GDTF>")
            .expect("XML");
        assert_eq!(root.name, "GDTF");
        assert_eq!(
            root.child("FixtureType").expect("it is there").get("Name"),
            "T"
        );
    }

    #[test]
    fn a_document_that_is_not_xml_is_nothing() {
        assert_eq!(parse(b"this is not xml"), None);
        assert_eq!(parse(b"<a><b></a>"), None);
        assert_eq!(parse(b"<a>"), None);
        assert_eq!(parse(b""), None);
    }

    #[test]
    fn a_document_that_nests_too_deep_is_nothing() {
        let deep = "<a>".repeat(MAX_DEPTH + 1) + &"</a>".repeat(MAX_DEPTH + 1);
        assert_eq!(parse(deep.as_bytes()), None);
        let shallow = "<a>".repeat(MAX_DEPTH - 1) + &"</a>".repeat(MAX_DEPTH - 1);
        assert!(parse(shallow.as_bytes()).is_some());
    }

    #[test]
    fn a_missing_attribute_is_the_empty_string() {
        let root = parse(b"<a/>").expect("it is XML");
        assert_eq!(root.get("missing"), "");
        assert_eq!(root.attribute("missing"), None);
        assert_eq!(root.child("missing"), None);
        assert_eq!(root.path(&["one", "two"]), None);
    }
}
