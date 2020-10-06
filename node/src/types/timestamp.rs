use std::{
    fmt::{self, Display, Formatter},
    ops::{Add, AddAssign, Div, Mul, Rem, Sub},
    str::FromStr,
    time::{Duration, SystemTime},
};

use datasize::DataSize;
use derive_more::{Add, AddAssign, From, Shl, Shr, Sub, SubAssign};
use humantime::{DurationError, TimestampError};
#[cfg(test)]
use rand::Rng;
use serde::{de::Error as SerdeError, Deserialize, Deserializer, Serialize, Serializer};

#[cfg(test)]
use crate::testing::TestRng;

/// A timestamp type, representing a concrete moment in time.
#[derive(DataSize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Shr, Shl)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Returns the timestamp of the current moment
    pub fn now() -> Self {
        let millis = SystemTime::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64;
        Timestamp(millis)
    }

    /// Returns the time that has elapsed since this timestamp
    pub fn elapsed(&self) -> TimeDiff {
        Timestamp::now() - *self
    }

    /// Returns a zero timestamp
    pub fn zero() -> Self {
        Timestamp(0)
    }

    /// Returns the timestamp as the number of milliseconds since the Unix epoch
    pub fn millis(&self) -> u64 {
        self.0
    }

    /// Returns the difference between `self` and `other`, or `0` if `self` is earlier than `other`.
    pub fn saturating_sub(self, other: Timestamp) -> TimeDiff {
        TimeDiff(self.0.saturating_sub(other.0))
    }

    /// Returns the number of trailing zeros in the number of milliseconds since the epoch.
    pub fn trailing_zeros(&self) -> u8 {
        self.0.trailing_zeros() as u8
    }

    /// Generates a random instance using a `TestRng`.
    #[cfg(test)]
    pub fn random(rng: &mut TestRng) -> Self {
        Timestamp(1_596_763_000_000 + rng.gen_range(200_000, 1_000_000))
    }
}

impl Display for Timestamp {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let system_time = SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_millis(self.0))
            .expect("should be within system time limits");
        write!(f, "{}", humantime::format_rfc3339_millis(system_time))
    }
}

impl FromStr for Timestamp {
    type Err = TimestampError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let system_time = humantime::parse_rfc3339_weak(value)?;
        let inner = system_time
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|_| TimestampError::OutOfRange)?
            .as_millis() as u64;
        Ok(Timestamp(inner))
    }
}

impl Sub<Timestamp> for Timestamp {
    type Output = TimeDiff;

    fn sub(self, other: Timestamp) -> TimeDiff {
        TimeDiff(self.0 - other.0)
    }
}

impl Add<TimeDiff> for Timestamp {
    type Output = Timestamp;

    fn add(self, diff: TimeDiff) -> Timestamp {
        Timestamp(self.0 + diff.0)
    }
}

impl AddAssign<TimeDiff> for Timestamp {
    fn add_assign(&mut self, rhs: TimeDiff) {
        self.0 += rhs.0;
    }
}

impl Sub<TimeDiff> for Timestamp {
    type Output = Timestamp;

    fn sub(self, diff: TimeDiff) -> Timestamp {
        Timestamp(self.0 - diff.0)
    }
}

impl Div<TimeDiff> for Timestamp {
    type Output = u64;

    fn div(self, rhs: TimeDiff) -> u64 {
        self.0 / rhs.0
    }
}

impl Rem<TimeDiff> for Timestamp {
    type Output = TimeDiff;

    fn rem(self, diff: TimeDiff) -> TimeDiff {
        TimeDiff(self.0 % diff.0)
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            self.to_string().serialize(serializer)
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            let value_as_string = String::deserialize(deserializer)?;
            Timestamp::from_str(&value_as_string).map_err(SerdeError::custom)
        } else {
            let inner = u64::deserialize(deserializer)?;
            Ok(Timestamp(inner))
        }
    }
}

#[cfg(test)]
impl From<u64> for Timestamp {
    fn from(arg: u64) -> Timestamp {
        Timestamp(arg)
    }
}

/// A time difference between two timestamps.
#[derive(
    Debug,
    Clone,
    Copy,
    DataSize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Add,
    AddAssign,
    Sub,
    SubAssign,
    From,
)]
pub struct TimeDiff(u64);

impl Display for TimeDiff {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", humantime::format_duration(Duration::from(*self)))
    }
}

impl FromStr for TimeDiff {
    type Err = DurationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let inner = humantime::parse_duration(value)?.as_millis() as u64;
        Ok(TimeDiff(inner))
    }
}

impl TimeDiff {
    /// Returns the timestamp as the number of milliseconds since the Unix epoch
    pub fn millis(&self) -> u64 {
        self.0
    }
}

impl Mul<u64> for TimeDiff {
    type Output = TimeDiff;

    fn mul(self, rhs: u64) -> TimeDiff {
        TimeDiff(self.0 * rhs)
    }
}

impl Div<u64> for TimeDiff {
    type Output = TimeDiff;

    fn div(self, rhs: u64) -> TimeDiff {
        TimeDiff(self.0 / rhs)
    }
}

impl From<TimeDiff> for Duration {
    fn from(diff: TimeDiff) -> Duration {
        Duration::from_millis(diff.0)
    }
}

impl Serialize for TimeDiff {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            self.to_string().serialize(serializer)
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for TimeDiff {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            let value_as_string = String::deserialize(deserializer)?;
            TimeDiff::from_str(&value_as_string).map_err(SerdeError::custom)
        } else {
            let inner = u64::deserialize(deserializer)?;
            Ok(TimeDiff(inner))
        }
    }
}

#[cfg(test)]
impl From<Duration> for TimeDiff {
    fn from(duration: Duration) -> TimeDiff {
        TimeDiff(duration.as_millis() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRng;

    #[test]
    fn timestamp_serialization_roundtrip() {
        let timestamp = Timestamp::now();

        let timestamp_as_string = timestamp.to_string();
        assert_eq!(
            timestamp,
            Timestamp::from_str(&timestamp_as_string).unwrap()
        );

        let serialized_json = serde_json::to_string(&timestamp).unwrap();
        assert_eq!(timestamp, serde_json::from_str(&serialized_json).unwrap());

        let serialized_rmp = rmp_serde::to_vec(&timestamp).unwrap();
        assert_eq!(
            timestamp,
            rmp_serde::from_read_ref(&serialized_rmp).unwrap()
        );
    }

    #[test]
    fn timediff_serialization_roundtrip() {
        let mut rng = TestRng::new();
        let timediff = TimeDiff(rng.gen());

        let timediff_as_string = timediff.to_string();
        assert_eq!(timediff, TimeDiff::from_str(&timediff_as_string).unwrap());

        let serialized_json = serde_json::to_string(&timediff).unwrap();
        assert_eq!(timediff, serde_json::from_str(&serialized_json).unwrap());

        let serialized_rmp = rmp_serde::to_vec(&timediff).unwrap();
        assert_eq!(timediff, rmp_serde::from_read_ref(&serialized_rmp).unwrap());
    }
}
