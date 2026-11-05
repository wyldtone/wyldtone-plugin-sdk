use serde::{Deserialize, Serialize};

use ringbuf::{
    HeapRb,
    traits::{Producer, Consumer, Split},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParameterValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    Choice(u32),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParameterKind {
    Float {
        min: f32,
        max: f32,
        step: Option<f32>,
    },

    Int {
        min: i32,
        max: i32,
        step: i32,
    },

    Bool,

    Choice {
        choices: Vec<String>,
    },
}

#[derive(Clone, Debug)]
pub struct Parameter {
    pub id: String,
    pub name: String,
    pub label: String,

    pub kind: ParameterKind,

    pub default: ParameterValue,
    pub value: ParameterValue,

    pub automatable: bool,
}

impl Parameter {
    pub fn range(&self) -> Option<(f32, f32)> {
        match &self.kind {
            ParameterKind::Float { min, max, .. } => {
                Some((*min, *max))
            }

            ParameterKind::Int { min, max, .. } => {
                Some((*min as f32, *max as f32))
            }

            ParameterKind::Bool => {
                Some((0.0, 1.0))
            }

            ParameterKind::Choice { choices } => {
                if choices.is_empty() {
                    None
                } else {
                    Some((0.0, (choices.len() - 1) as f32))
                }
            }
        }
    }

    pub fn normalized_value(&self) -> f32 {
        match (&self.kind, &self.value) {
            (
                ParameterKind::Float { min, max, .. },
                ParameterValue::Float(value),
            ) => {
                if max <= min {
                    return 0.0;
                }

                ((value.clamp(*min, *max) - min)
                    / (max - min))
                    .clamp(0.0, 1.0)
            }

            (
                ParameterKind::Int { min, max, .. },
                ParameterValue::Int(value),
            ) => {
                if max <= min {
                    return 0.0;
                }

                ((*value).clamp(*min, *max) - min) as f32
                    / (*max - *min) as f32
            }

            (
                ParameterKind::Bool,
                ParameterValue::Bool(value),
            ) => {
                if *value {
                    1.0
                } else {
                    0.0
                }
            }

            (
                ParameterKind::Choice { choices },
                ParameterValue::Choice(index),
            ) => {
                if choices.len() <= 1 {
                    return 0.0;
                }

                let max_index = choices.len() - 1;
                let index = (*index as usize).min(max_index);

                index as f32 / max_index as f32
            }

            _ => 0.0,
        }
    }

    pub fn default_normalized_value(&self) -> f32 {
        let mut parameter = self.clone();

        parameter.value = self.default.clone();

        parameter.normalized_value()
    }

    pub fn value_from_normalized(
        &self,
        normalized: f32,
    ) -> ParameterValue {
        let normalized = normalized.clamp(0.0, 1.0);

        match &self.kind {
            ParameterKind::Float { min, max, .. } => {
                ParameterValue::Float(
                    min + normalized * (max - min)
                )
            }

            ParameterKind::Int { min, max, .. } => {
                let value =
                    *min as f32
                    + normalized * (*max - *min) as f32;

                ParameterValue::Int(value.round() as i32)
            }

            ParameterKind::Bool => {
                ParameterValue::Bool(normalized >= 0.5)
            }

            ParameterKind::Choice { choices } => {
                if choices.is_empty() {
                    return ParameterValue::Choice(0);
                }

                if choices.len() == 1 {
                    return ParameterValue::Choice(0);
                }

                let max_index = choices.len() - 1;

                let index =
                    (normalized * max_index as f32).round()
                        as usize;

                ParameterValue::Choice(
                    index.min(max_index) as u32
                )
            }
        }
    }

    pub fn set_value(&mut self, value: ParameterValue) {
        match (&self.kind, value) {
            (
                ParameterKind::Float { min, max, .. },
                ParameterValue::Float(value),
            ) => {
                self.value =
                    ParameterValue::Float(value.clamp(*min, *max));
            }

            (
                ParameterKind::Int { min, max, .. },
                ParameterValue::Int(value),
            ) => {
                self.value =
                    ParameterValue::Int(value.clamp(*min, *max));
            }

            (
                ParameterKind::Bool,
                ParameterValue::Bool(value),
            ) => {
                self.value = ParameterValue::Bool(value);
            }

            (
                ParameterKind::Choice { choices },
                ParameterValue::Choice(index),
            ) => {
                let max_index =
                    choices.len().saturating_sub(1);

                self.value =
                    ParameterValue::Choice(
                        (index as usize).min(max_index) as u32
                    );
            }

            // A value of the wrong type must never silently
            // replace the current parameter value.
            _ => {}
        }
    }

    pub fn set_normalized(&mut self, normalized: f32) {
        let value = self.value_from_normalized(normalized);
        self.set_value(value);
    }

    pub fn reset(&mut self) {
        let default = self.default.clone();
        self.set_value(default);
    }

    pub fn is_valid(&self) -> bool {
        if self.id.is_empty() {
            return false;
        }

        match (&self.kind, &self.default, &self.value) {
            (
                ParameterKind::Float { min, max, .. },
                ParameterValue::Float(default),
                ParameterValue::Float(value),
            ) => {
                min <= max
                    && default >= min
                    && default <= max
                    && value >= min
                    && value <= max
            }

            (
                ParameterKind::Int { min, max, step },
                ParameterValue::Int(default),
                ParameterValue::Int(value),
            ) => {
                min <= max
                    && *step > 0
                    && default >= min
                    && default <= max
                    && value >= min
                    && value <= max
            }

            (
                ParameterKind::Bool,
                ParameterValue::Bool(_),
                ParameterValue::Bool(_),
            ) => true,

            (
                ParameterKind::Choice { choices },
                ParameterValue::Choice(default),
                ParameterValue::Choice(value),
            ) => {
                !choices.is_empty()
                    && (*default as usize) < choices.len()
                    && (*value as usize) < choices.len()
            }

            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParameterId {
    pub plugin_index: usize,
    pub parameter_id: String,
}

impl ParameterId {
    pub fn new(plugin_index: usize, parameter_id: impl Into<String>) -> Self {
        Self {
            plugin_index,
            parameter_id: parameter_id.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParameterChange {
    pub parameter_id: ParameterId,
    pub value: f32,
}

impl ParameterChange {
    pub fn new(
        parameter_id: ParameterId,
        value: f32,
    ) -> Self {
        Self {
            parameter_id,
            value,
        }
    }
}

pub struct ParameterQueue {
    producer: ringbuf::HeapProd<ParameterChange>,
    consumer: ringbuf::HeapCons<ParameterChange>,
}

impl ParameterQueue {
    pub fn new(size: usize) -> Self {
        let buffer = HeapRb::<ParameterChange>::new(size);
        let (producer, consumer) = buffer.split();

        Self {
            producer,
            consumer,
        }
    }

    pub fn split(self) -> (ParameterProducer, ParameterConsumer) {
        (
            ParameterProducer {
                producer: self.producer,
            },
            ParameterConsumer {
                consumer: self.consumer,
            },
        )
    }
}

pub struct ParameterProducer {
    producer: ringbuf::HeapProd<ParameterChange>,
}

impl ParameterProducer {
    pub fn push(&mut self, change: ParameterChange) -> bool {
        self.producer.try_push(change).is_ok()
    }
}

pub struct ParameterConsumer {
    consumer: ringbuf::HeapCons<ParameterChange>,
}

impl ParameterConsumer {
    pub fn pop(&mut self) -> Option<ParameterChange> {
        self.consumer.try_pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_parameter() -> Parameter {
        Parameter {
            id: "drive".to_string(),
            name: "Drive".to_string(),
            label: "x".to_string(),

            kind: ParameterKind::Float {
                min: 0.0,
                max: 10.0,
                step: Some(0.1),
            },

            default: ParameterValue::Float(2.0),
            value: ParameterValue::Float(2.0),

            automatable: true,
        }
    }

    #[test]
    fn set_value_clamps_to_range() {
        let mut parameter = test_parameter();

        parameter.set_value(ParameterValue::Float(15.0));

        assert_eq!(
            parameter.value,
            ParameterValue::Float(10.0)
        );

        parameter.set_value(ParameterValue::Float(-5.0));

        assert_eq!(
            parameter.value,
            ParameterValue::Float(0.0)
        );
    }

    #[test]
    fn normalized_value_works() {
        let parameter = test_parameter();

        assert_eq!(parameter.normalized_value(), 0.2);
    }

    #[test]
    fn set_normalized_works() {
        let mut parameter = test_parameter();

        parameter.set_normalized(0.4);

        assert_eq!(
            parameter.value,
            ParameterValue::Float(4.0)
        );
    }

    #[test]
    fn reset_restores_default() {
        let mut parameter = test_parameter();

        parameter.set_value(ParameterValue::Float(8.0));

        assert_eq!(
            parameter.value,
            ParameterValue::Float(8.0)
        );

        parameter.reset();

        assert_eq!(
            parameter.value,
            ParameterValue::Float(2.0)
        );
    }

    #[test]
    fn int_parameter_normalization_works() {
        let parameter = Parameter {
            id: "oversampling".to_string(),
            name: "Oversampling".to_string(),
            label: "x".to_string(),

            kind: ParameterKind::Int {
                min: 1,
                max: 4,
                step: 1,
            },

            default: ParameterValue::Int(2),
            value: ParameterValue::Int(2),

            automatable: true,
        };

        assert!(
            (parameter.normalized_value() - (1.0 / 3.0)).abs()
                < 1e-6
        );
    }

    #[test]
    fn int_parameter_rounds_normalized_value() {
        let mut parameter = Parameter {
            id: "oversampling".to_string(),
            name: "Oversampling".to_string(),
            label: "x".to_string(),

            kind: ParameterKind::Int {
                min: 1,
                max: 4,
                step: 1,
            },

            default: ParameterValue::Int(1),
            value: ParameterValue::Int(1),

            automatable: true,
        };

        parameter.set_normalized(0.5);

        assert_eq!(
            parameter.value,
            ParameterValue::Int(3)
        );
    }

    #[test]
    fn bool_parameter_normalization_works() {
        let mut parameter = Parameter {
            id: "bypass".to_string(),
            name: "Bypass".to_string(),
            label: "".to_string(),

            kind: ParameterKind::Bool,

            default: ParameterValue::Bool(false),
            value: ParameterValue::Bool(false),

            automatable: true,
        };

        assert_eq!(parameter.normalized_value(), 0.0);

        parameter.set_normalized(1.0);

        assert_eq!(
            parameter.value,
            ParameterValue::Bool(true)
        );
    }

    #[test]
    fn choice_parameter_works() {
        let mut parameter = Parameter {
            id: "mode".to_string(),
            name: "Mode".to_string(),
            label: "".to_string(),

            kind: ParameterKind::Choice {
                choices: vec![
                    "Clean".to_string(),
                    "Crunch".to_string(),
                    "High Gain".to_string(),
                ],
            },

            default: ParameterValue::Choice(0),
            value: ParameterValue::Choice(0),

            automatable: true,
        };

        assert_eq!(parameter.normalized_value(), 0.0);

        parameter.set_normalized(1.0);

        assert_eq!(
            parameter.value,
            ParameterValue::Choice(2)
        );

        parameter.set_normalized(0.5);

        assert_eq!(
            parameter.value,
            ParameterValue::Choice(1)
        );
    }

    #[test]
    fn choice_parameter_rejects_invalid_index() {
        let mut parameter = Parameter {
            id: "mode".to_string(),
            name: "Mode".to_string(),
            label: "".to_string(),

            kind: ParameterKind::Choice {
                choices: vec![
                    "Clean".to_string(),
                    "Crunch".to_string(),
                    "High Gain".to_string(),
                ],
            },

            default: ParameterValue::Choice(0),
            value: ParameterValue::Choice(0),

            automatable: true,
        };

        parameter.set_value(ParameterValue::Choice(100));

        assert_eq!(
            parameter.value,
            ParameterValue::Choice(2)
        );

        assert!(parameter.is_valid());
    }

    #[test]
    fn parameter_change_survives_queue() {
        let queue = ParameterQueue::new(8);
        let (mut producer, mut consumer) = queue.split();

        let parameter_id = ParameterId::new(2, "drive");

        let change = ParameterChange::new(
            parameter_id.clone(),
            4.0,
        );

        assert!(producer.push(change));

        let received = consumer.pop().unwrap();

        assert_eq!(received.parameter_id, parameter_id);
        assert_eq!(received.value, 4.0);
    }

    #[test]
    fn valid_parameter_is_valid() {
        let parameter = test_parameter();

        assert!(parameter.is_valid());
    }

    #[test]
    fn invalid_parameter_is_rejected() {
        let mut parameter = test_parameter();

        parameter.id.clear();
        assert!(!parameter.is_valid());

        parameter.id = "drive".to_string();
        parameter.kind = ParameterKind::Float {
            min: 10.0,
            max: 0.0,
            step: None,
        };
        assert!(!parameter.is_valid());

        parameter.kind = ParameterKind::Float {
            min: 0.0,
            max: 10.0,
            step: None,
        };

        parameter.default = ParameterValue::Float(20.0);
        assert!(!parameter.is_valid());
    }

    #[test]
    fn parameter_ids_must_be_unique() {
        let parameter_a = test_parameter();

        let mut parameter_b = test_parameter();
        parameter_b.id = "tone".to_string();

        let parameters = vec![parameter_a, parameter_b];

        let mut ids = std::collections::HashSet::new();

        for parameter in &parameters {
            assert!(ids.insert(&parameter.id));
        }
    }

    #[test]
    #[should_panic]
    fn duplicate_parameter_ids_are_rejected() {
        let parameter_a = test_parameter();
        let parameter_b = test_parameter();

        let parameters = vec![parameter_a, parameter_b];

        let mut ids = std::collections::HashSet::new();

        for parameter in &parameters {
            assert!(
                ids.insert(&parameter.id),
                "duplicate parameter ID: {}",
                parameter.id
            );
        }
    }
}