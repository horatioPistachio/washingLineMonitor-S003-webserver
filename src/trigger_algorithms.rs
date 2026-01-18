use signalo::filters::mean::mean;
use signalo::traits::Filter;

pub fn is_stable_resistance(resistances: &[f64], stability_threshold: f64) -> bool {
    if resistances.is_empty() {
        return false;
    }

    if resistances.len() < 3 {
        return false;
    }

    let smoothed_values: Vec<_> = resistances
        .iter()
        .scan(mean::Mean::<f64, 3>::default(), |filter1, &resistance| {
            let output = filter1.filter(resistance);
            Some(output)
        })
        .collect();

    let first_value = smoothed_values.first().unwrap();

    if first_value.abs() < f64::EPSILON {
        return false; // Guard against division by zero
    }
    let last_value = smoothed_values.last().unwrap();

    let derivative = (last_value - first_value) / (smoothed_values.len() as f64);
    let normalized_derivative = derivative / first_value;
    return normalized_derivative.abs() < stability_threshold;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stable_resistance() {
        let stable = vec![100.0, 100.1, 99.9, 100.05];
        assert!(is_stable_resistance(&stable, 0.01));
    }

    #[test]
    fn test_unstable_resistance() {
        let unstable = vec![100.0, 110.0, 120.0, 130.0];
        assert!(!is_stable_resistance(&unstable, 0.01));
    }

    #[test]
    fn test_empty_input() {
        assert!(!is_stable_resistance(&[], 0.01));
    }

    #[test]
    fn test_insufficient_samples() {
        assert!(!is_stable_resistance(&[100.0, 101.0], 0.01));
    }
}
