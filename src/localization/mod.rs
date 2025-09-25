use fluent::{FluentBundle, FluentResource};
use unic_langid::langid;

pub fn test() {
    let ftl_string = "hello-world = Hello, world!".to_owned();
    let res = FluentResource::try_new(ftl_string).expect("Failed to parse an FTL string.");

    let langid_en = langid!("en-US");
    let mut bundle = FluentBundle::new(vec![langid_en]);

    bundle
        .add_resource(&res)
        .expect("Failed to add FTL resources to the bundle.");

    let msg = bundle
        .get_message("hello-world")
        .expect("Message doesn't exist.");
    let mut errors = vec![];
    let pattern = msg.value().expect("Message has no value.");
    let value = bundle.format_pattern(&pattern, None, &mut errors);

    assert_eq!(&value, "Hello, world!");
}
