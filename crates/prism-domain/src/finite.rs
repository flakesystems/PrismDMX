//! Serde guards that keep NaN and infinity off the wire.
//!
//! Every `f64` in this crate is a physical quantity — a fade time, an angle, a
//! coordinate. None of them has a meaningful non-finite value, and both wire
//! formats punish one:
//!
//! - JSON cannot encode NaN or infinity at all. `serde_json` writes `null`
//!   instead — silently — and reading that back into an `f64` fails. A show
//!   file saved with one NaN in it does not reopen, and nothing warned when it
//!   was saved.
//! - MessagePack *can* encode both, faithfully. An infinite fade time would
//!   therefore survive the IPC wire intact and produce a cue that never
//!   completes, and a NaN would compare unequal to itself, which breaks every
//!   equality the UI and the Oops journal depend on.
//!
//! `serde_json` does reject an out-of-range literal such as `1e400` on its own,
//! so the read guard is aimed at MessagePack, which does not. The write guard is
//! aimed at both.
//!
//! These helpers make the boundary explicit in both directions. A non-finite
//! value is rejected when it is written and when it is read, so a domain value
//! either round-trips exactly or fails loudly. It is never silently corrupted.
//!
//! Callers must treat serialisation as fallible: `prism-ipc` and the show writer
//! report the error rather than emitting a broken frame. That is the point — the
//! alternative is a cue with an infinite fade discovered mid-show.

use serde::{Deserialize as _, Deserializer, Serializer, de::Error as _, ser::Error as _};

/// Message used for both directions, so the cause is recognisable either way.
fn rejection(value: f64) -> String {
    format!("expected a finite number, got {value}")
}

/// Serialises a finite `f64`, failing on NaN and infinity.
///
/// # Errors
///
/// Fails if `value` is not finite.
pub fn serialize<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
    if !value.is_finite() {
        return Err(S::Error::custom(rejection(*value)));
    }
    serializer.serialize_f64(*value)
}

/// Deserialises an `f64`, rejecting NaN and infinity.
///
/// # Errors
///
/// Fails if the encoded value is not finite. MessagePack can carry NaN and
/// infinity, so this is the guard that keeps them off the IPC wire; `serde_json`
/// already rejects an out-of-range literal itself.
pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    let value = f64::deserialize(deserializer)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(D::Error::custom(rejection(value)))
    }
}

/// The same guard for an optional value.
pub mod option {
    use serde::{Deserialize as _, Deserializer, Serializer, de::Error as _, ser::Error as _};

    /// Serialises an optional finite `f64`, failing on NaN and infinity.
    ///
    /// # Errors
    ///
    /// Fails if the value is present and not finite.
    pub fn serialize<S: Serializer>(value: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(number) if !number.is_finite() => Err(S::Error::custom(super::rejection(*number))),
            Some(number) => serializer.serialize_some(number),
            None => serializer.serialize_none(),
        }
    }

    /// Deserialises an optional `f64`, rejecting NaN and infinity.
    ///
    /// # Errors
    ///
    /// Fails if the value is present and not finite.
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<f64>, D::Error> {
        let value = Option::<f64>::deserialize(deserializer)?;
        match value {
            Some(number) if !number.is_finite() => Err(D::Error::custom(super::rejection(number))),
            other => Ok(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Cue, CueTrigger, Vec3};

    fn vec3(x: f64) -> Vec3 {
        Vec3::new(x, 0.0, 0.0)
    }

    #[test]
    fn a_non_finite_value_cannot_be_written() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let error = serde_json::to_string(&vec3(value)).unwrap_err();
            assert!(
                error.to_string().contains("expected a finite number"),
                "{error}"
            );
            assert!(rmp_serde::to_vec_named(&vec3(value)).is_err());
        }
    }

    #[test]
    fn a_finite_value_is_written_unchanged() {
        assert_eq!(
            serde_json::to_string(&vec3(-1.5)).unwrap(),
            r#"{"x":-1.5,"y":0.0,"z":0.0}"#
        );
    }

    #[test]
    fn json_null_for_a_float_is_rejected() {
        // This is what serde_json would have written for a NaN, so a file from a
        // build without the guard is caught rather than half-loaded.
        assert!(serde_json::from_str::<Vec3>(r#"{"x":null,"y":0,"z":0}"#).is_err());
    }

    #[test]
    fn a_non_finite_value_on_the_messagepack_wire_is_rejected() {
        // MessagePack encodes IEEE 754 directly, so infinity and NaN survive the
        // wire intact. This is the case the read guard exists for: without it, a
        // corrupt or hostile frame could set a fade time that never completes.
        #[derive(serde::Serialize)]
        struct Unguarded {
            x: f64,
            y: f64,
            z: f64,
        }

        for value in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let packed = rmp_serde::to_vec_named(&Unguarded {
                x: value,
                y: 0.0,
                z: 0.0,
            })
            .expect("the unguarded helper encodes anything");
            let error = rmp_serde::from_slice::<Vec3>(&packed).unwrap_err();
            assert!(
                error.to_string().contains("expected a finite number"),
                "{error}"
            );
        }
    }

    #[test]
    fn serde_json_rejects_an_overflowing_literal_before_the_guard_sees_it() {
        // Documents what the read guard does *not* have to cover on the JSON
        // side, so the division of responsibility stays visible.
        assert!(serde_json::from_str::<Vec3>(r#"{"x":1e400,"y":0,"z":0}"#).is_err());
    }

    #[test]
    fn the_guard_covers_optional_fields_too() {
        let mut cue = Cue {
            number: "1".to_owned(),
            name: String::new(),
            fade_in: 0.0,
            fade_out: 0.0,
            delay: 0.0,
            trigger: CueTrigger::Time,
            trigger_time: Some(f64::INFINITY),
            parts: Vec::new(),
        };
        assert!(
            serde_json::to_string(&cue)
                .unwrap_err()
                .to_string()
                .contains("finite")
        );

        cue.trigger_time = None;
        let json = serde_json::to_string(&cue).unwrap();
        assert!(json.contains(r#""triggerTime":null"#));
        assert_eq!(serde_json::from_str::<Cue>(&json).unwrap(), cue);

        assert!(
            serde_json::from_str::<Cue>(
                &json.replace("\"triggerTime\":null", "\"triggerTime\":1e400")
            )
            .is_err()
        );
    }
}
