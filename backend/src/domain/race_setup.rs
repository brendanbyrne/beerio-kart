use crate::error::AppError;

/// A fully-specified race setup update (all four IDs required together).
///
/// The API accepts `preferred_*_id` fields individually for partial updates,
/// but a race setup must always be all-four-or-nothing: picking a character
/// without wheels (etc.) is meaningless. This type encodes that invariant at
/// compile time so call sites can't read only some of the fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaceSetupUpdate {
    pub character_id: i32,
    pub body_id: i32,
    pub wheel_id: i32,
    pub glider_id: i32,
}

impl RaceSetupUpdate {
    /// Parse a race-setup update from four optional fields.
    ///
    /// - All four `None` → `Ok(None)` (caller is not updating race setup).
    /// - All four `Some` → `Ok(Some(RaceSetupUpdate))`.
    /// - Any mix        → `Err(BadRequest)`.
    pub fn try_from_optional(
        character_id: Option<i32>,
        body_id: Option<i32>,
        wheel_id: Option<i32>,
        glider_id: Option<i32>,
    ) -> Result<Option<Self>, AppError> {
        match (character_id, body_id, wheel_id, glider_id) {
            (None, None, None, None) => Ok(None),
            (Some(character_id), Some(body_id), Some(wheel_id), Some(glider_id)) => {
                Ok(Some(Self {
                    character_id,
                    body_id,
                    wheel_id,
                    glider_id,
                }))
            }
            _ => Err(AppError::BadRequest(
                "Race setup must be provided all together (character, body, wheel, glider) or not at all".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_none_returns_ok_none() {
        let result = RaceSetupUpdate::try_from_optional(None, None, None, None).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn all_some_returns_ok_some() {
        let result = RaceSetupUpdate::try_from_optional(Some(1), Some(2), Some(3), Some(4))
            .unwrap()
            .unwrap();
        assert_eq!(
            result,
            RaceSetupUpdate {
                character_id: 1,
                body_id: 2,
                wheel_id: 3,
                glider_id: 4,
            }
        );
    }

    fn assert_bad_request(result: Result<Option<RaceSetupUpdate>, AppError>) {
        match result {
            Err(AppError::BadRequest(msg)) => {
                assert!(msg.contains("all together"), "msg was: {msg}");
            }
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[test]
    fn single_some_is_bad_request() {
        assert_bad_request(RaceSetupUpdate::try_from_optional(
            Some(1),
            None,
            None,
            None,
        ));
    }

    #[test]
    fn three_some_one_none_is_bad_request() {
        assert_bad_request(RaceSetupUpdate::try_from_optional(
            Some(1),
            Some(2),
            Some(3),
            None,
        ));
    }

    #[test]
    fn interior_none_is_bad_request() {
        assert_bad_request(RaceSetupUpdate::try_from_optional(
            Some(1),
            None,
            Some(3),
            Some(4),
        ));
    }
}
