use uuid::Uuid;

use super::extractor::{KnownRoomType, RawOcrItem, TextAnnotation, TextAnnotationType};
use crate::blueprint::LengthUnit;

/// Convert raw OCR items into classified `TextAnnotation`s (FR-021, FR-022, FR-023).
pub fn parse_annotations(raw: &[RawOcrItem]) -> Vec<TextAnnotation> {
    raw.iter().map(parse_item).collect()
}

fn parse_item(item: &RawOcrItem) -> TextAnnotation {
    let annotation_type = if item.confidence < 0.50 {
        TextAnnotationType::Unreadable
    } else if let Some(dim) = parse_dimension(&item.text) {
        TextAnnotationType::DimensionValue {
            value: dim.0,
            unit: dim.1,
        }
    } else if let Some(rt) = match_room_type(&item.text) {
        TextAnnotationType::RoomLabel(rt)
    } else if item.text.len() >= 3 {
        TextAnnotationType::RoomLabelUnknown
    } else {
        TextAnnotationType::Unreadable
    };

    TextAnnotation {
        id: Uuid::new_v4(),
        raw_text: item.text.clone(),
        annotation_type,
        image_bounds: item.bounds,
        confidence: item.confidence,
    }
}

/// Match text against known room types (case-insensitive) (FR-021).
///
/// Handles both full multi-word labels ("LIVING ROOM") and unambiguous single-word
/// fragments ("LIVING", "DINING") that OCR may produce from multi-word labels.
pub fn match_room_type(text: &str) -> Option<KnownRoomType> {
    let upper = text.to_uppercase();
    let upper = upper.trim();
    // Multi-word and keyword matches
    if upper.contains("LIVING") {
        return Some(KnownRoomType::LivingRoom);
    }
    if upper.contains("DINING") {
        return Some(KnownRoomType::DiningRoom);
    }
    if upper.contains("BEDROOM") || upper.contains("BED RM") || upper.contains("BED ROOM") {
        return Some(KnownRoomType::Bedroom);
    }
    if upper.contains("BATHROOM") || upper.contains("BATH ROOM") {
        return Some(KnownRoomType::Bathroom);
    }
    if upper.contains("LAUNDRY") {
        return Some(KnownRoomType::Laundry);
    }
    match upper {
        "BEDROOM" | "BED" | "MASTER BED" | "MASTER BEDROOM" => Some(KnownRoomType::Bedroom),
        "KITCHEN" | "KIT" | "KITCH" => Some(KnownRoomType::Kitchen),
        "BATH" | "RESTROOM" | "WC" | "TOILET" => Some(KnownRoomType::Bathroom),
        "GARAGE" | "CAR GARAGE" => Some(KnownRoomType::Garage),
        "HALLWAY" | "HALL" | "CORRIDOR" => Some(KnownRoomType::Hallway),
        "STUDY" | "OFFICE" | "DEN" => Some(KnownRoomType::Study),
        "UTILITY" => Some(KnownRoomType::Laundry),
        _ => None,
    }
}

/// Parse dimension strings (FR-022):
/// - `3.66m` / `3.66 m` → (3.66, Meters)
/// - `3660mm` → (3.66, Meters)
/// - `12'` / `12 ft` / `12'-6"` → (converted to meters, Feet internally then meters)
/// - `15'` → 15 feet = 4.572 m stored as (4.572, Meters) ? No — store as feet.
///
/// Returns `None` if the string doesn't match any dimension pattern.
pub fn parse_dimension(text: &str) -> Option<(f64, LengthUnit)> {
    let t = text.trim();

    // Meters: "3.66m" or "3.66 m"
    if let Some(num) = t.strip_suffix("m").or_else(|| t.strip_suffix(" m")) {
        if let Ok(v) = num.trim().parse::<f64>() {
            return Some((v, LengthUnit::Meters));
        }
    }

    // Millimeters: "3660mm"
    if let Some(num) = t.strip_suffix("mm") {
        if let Ok(v) = num.trim().parse::<f64>() {
            return Some((v / 1000.0, LengthUnit::Meters));
        }
    }

    // Centimeters: "366cm"
    if let Some(num) = t.strip_suffix("cm") {
        if let Ok(v) = num.trim().parse::<f64>() {
            return Some((v / 100.0, LengthUnit::Meters));
        }
    }

    // Feet-inches: "12'-6\"" or "12'" or "12 ft" or "12 feet"
    let t_lower = t.to_lowercase();
    if t_lower.ends_with("ft") || t_lower.ends_with("feet") || t.contains('\'') {
        return parse_feet_inches(t);
    }

    None
}

/// Parse feet-inches notation: `12'-6"`, `12'`, `12 ft`, `12.5 feet`.
fn parse_feet_inches(t: &str) -> Option<(f64, LengthUnit)> {
    // Remove suffix variants
    let cleaned = t.replace("feet", "").replace("ft", "").trim().to_string();

    // "12'-6\"" pattern (dash is separator between feet and inches, not minus)
    if cleaned.contains('\'') && cleaned.contains('"') {
        let parts: Vec<&str> = cleaned.split('\'').collect();
        let feet: f64 = parts[0].trim().parse().ok()?;
        // Strip leading '-' separator and trailing '"'
        let inches_str = parts[1]
            .replace('"', "")
            .trim_start_matches('-')
            .trim()
            .to_string();
        let inches: f64 = inches_str.parse().unwrap_or(0.0);
        return Some((feet + inches / 12.0, LengthUnit::Feet));
    }

    // "12'" pattern
    if let Some(num) = cleaned.strip_suffix('\'') {
        if let Ok(v) = num.trim().parse::<f64>() {
            return Some((v, LengthUnit::Feet));
        }
    }

    // Plain number that was tagged as feet
    if let Ok(v) = cleaned.trim().parse::<f64>() {
        return Some((v, LengthUnit::Feet));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_bedroom_label() {
        assert_eq!(match_room_type("BEDROOM"), Some(KnownRoomType::Bedroom));
        assert_eq!(match_room_type("bedroom"), Some(KnownRoomType::Bedroom));
    }

    #[test]
    fn match_living_room_label() {
        assert_eq!(
            match_room_type("LIVING ROOM"),
            Some(KnownRoomType::LivingRoom)
        );
    }

    #[test]
    fn match_kitchen_label() {
        assert_eq!(match_room_type("KITCHEN"), Some(KnownRoomType::Kitchen));
    }

    #[test]
    fn parse_meters_dimension() {
        let result = parse_dimension("3.66m").unwrap();
        assert!((result.0 - 3.66).abs() < 0.001);
        assert_eq!(result.1, LengthUnit::Meters);
    }

    #[test]
    fn parse_millimeters_dimension() {
        let result = parse_dimension("3660mm").unwrap();
        assert!((result.0 - 3.66).abs() < 0.001);
    }

    #[test]
    fn parse_feet_dimension() {
        let result = parse_dimension("15'").unwrap();
        assert!((result.0 - 15.0).abs() < 0.001);
        assert_eq!(result.1, LengthUnit::Feet);
    }

    #[test]
    fn parse_feet_inches_dimension() {
        let result = parse_dimension("12'-6\"").unwrap();
        assert!((result.0 - 12.5).abs() < 0.01);
        assert_eq!(result.1, LengthUnit::Feet);
    }

    #[test]
    fn parse_unknown_returns_none() {
        assert!(parse_dimension("BEDROOM").is_none());
        assert!(parse_dimension("").is_none());
    }
}
