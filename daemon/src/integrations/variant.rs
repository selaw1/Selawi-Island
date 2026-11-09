//! Reading loosely typed D-Bus property maps (`a{sv}`) safely: a missing
//! or wrongly typed value is `None`, never a crash.

use std::collections::HashMap;

use zbus::{OwnedVariant, Variant};

pub type Props = HashMap<String, OwnedVariant>;

/// Some services wrap values in an extra variant layer.
fn inner<'v, 'a>(v: &'v Variant<'a>) -> &'v Variant<'a> {
    match v {
        Variant::Variant(boxed) => inner(boxed),
        other => other,
    }
}

fn get<'p>(props: &'p Props, key: &str) -> Option<&'p Variant<'static>> {
    props.get(key).map(|v| inner(v))
}

pub fn string(props: &Props, key: &str) -> Option<String> {
    String::try_from(get(props, key)?)
        .ok()
        .filter(|s| !s.trim().is_empty())
}

pub fn strings(props: &Props, key: &str) -> Vec<String> {
    get(props, key)
        .and_then(|v| v.try_to_owned().ok())
        .and_then(|owned| Vec::<String>::try_from(owned).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .collect()
}

pub fn boolean(props: &Props, key: &str) -> Option<bool> {
    bool::try_from(get(props, key)?).ok()
}

pub fn u32_(props: &Props, key: &str) -> Option<u32> {
    u32::try_from(get(props, key)?).ok()
}

pub fn f64_(props: &Props, key: &str) -> Option<f64> {
    f64::try_from(get(props, key)?).ok()
}

pub fn u8_(props: &Props, key: &str) -> Option<u8> {
    u8::try_from(get(props, key)?).ok()
}

/// Signed or unsigned 64-bit integers (MPRIS lengths come as either).
pub fn i64_(props: &Props, key: &str) -> Option<i64> {
    let v = get(props, key)?;
    i64::try_from(v)
        .ok()
        .or_else(|| u64::try_from(v).ok().and_then(|u| i64::try_from(u).ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(entries: Vec<(&str, Variant<'static>)>) -> Props {
        entries
            .into_iter()
            .filter_map(|(k, v)| v.try_into_owned().ok().map(|o| (k.to_string(), o)))
            .collect()
    }

    #[test]
    fn reads_values_and_tolerates_wrong_types() {
        let p = props(vec![
            ("title", Variant::from("Song")),
            ("blank", Variant::from("  ")),
            ("len", Variant::U64(240_000_000)),
            ("signed", Variant::I64(-1)),
            ("wrapped", Variant::Variant(Box::new(Variant::from(true)))),
            ("artists", Variant::from(vec!["A", "", "B"])),
        ]);
        assert_eq!(string(&p, "title").as_deref(), Some("Song"));
        assert_eq!(string(&p, "blank"), None);
        assert_eq!(string(&p, "len"), None, "wrong type is None");
        assert_eq!(string(&p, "missing"), None);
        assert_eq!(i64_(&p, "len"), Some(240_000_000));
        assert_eq!(i64_(&p, "signed"), Some(-1));
        assert_eq!(boolean(&p, "wrapped"), Some(true));
        assert_eq!(
            strings(&p, "artists"),
            vec!["A".to_string(), "B".to_string()]
        );
        assert!(strings(&p, "title").is_empty());
    }
}
