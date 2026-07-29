//! Player-facing historical source catalogue.
//!
//! The packaged Markdown remains the repository's archival copy. These
//! sections put the same citations inside the game where players can read
//! them without leaving the menu.

pub const SECTION_COUNT: usize = 3;

pub fn section_label(section: usize) -> &'static str {
    match section {
        0 => "Primary Documents",
        1 => "Books & Scholarship",
        _ => "Sites & Archives",
    }
}

pub fn section_lines(section: usize) -> &'static [&'static str] {
    match section {
        0 => &[
            "PRIMARY SOURCES — TREATIES AND GOVERNMENT DOCUMENTS",
            "",
            "Medicine Lodge Treaty, 1867",
            "National Archives and Records Administration.",
            "",
            "Treaty of Fort Laramie, 1868",
            "National Archives and Records Administration.",
            "",
            "These records ground the campaign's treaty language, dates, and federal policy.",
        ],
        1 => &[
            "BOOKS AND HISTORICAL SCHOLARSHIP",
            "",
            "Stephen E. Ambrose, Nothing Like It in the World: The Men Who Built the",
            "Transcontinental Railroad, 1863–1869. Simon & Schuster, 2000.",
            "",
            "William H. Leckie, The Military Conquest of the Southern Plains.",
            "University of Oklahoma Press, 1963.",
            "",
            "Mildred P. Mayhall, The Kiowa. University of Oklahoma Press, 1962.",
            "",
            "Robert M. Utley, The Lance and the Shield: The Life and Times of Sitting Bull.",
            "Henry Holt, 1993.",
        ],
        _ => &[
            "HISTORICAL SITES, MUSEUMS, AND REFERENCE COLLECTIONS",
            "",
            "Promontory Summit, May 10, 1869 — National Park Service,",
            "Golden Spike National Historical Park.",
            "",
            "Adobe Walls, June 27, 1874 — Texas State Historical Association.",
            "The Red River War — Texas State Historical Association.",
            "The Transcontinental Railroad — Stephen E. Ambrose.",
            "Fort Marion Prisoners, 1875–1878 — Florida Museum of Natural History.",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_contains_real_citations_not_a_file_pointer() {
        let catalogue = (0..SECTION_COUNT)
            .flat_map(section_lines)
            .copied()
            .collect::<Vec<_>>()
            .join("\n");

        for expected in [
            "Medicine Lodge Treaty",
            "Treaty of Fort Laramie",
            "Stephen E. Ambrose",
            "William H. Leckie",
            "Mildred P. Mayhall",
            "Robert M. Utley",
            "Promontory Summit",
            "Adobe Walls",
            "The Red River War",
            "The Transcontinental Railroad",
            "Fort Marion Prisoners",
        ] {
            assert!(catalogue.contains(expected), "missing citation: {expected}");
        }
        assert!(!catalogue.contains(".md"));
        assert!(!catalogue.contains("install folder"));
    }

    #[test]
    fn category_labels_are_distinct_and_complete() {
        let labels = (0..SECTION_COUNT)
            .map(section_label)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(labels.len(), SECTION_COUNT);
    }
}
