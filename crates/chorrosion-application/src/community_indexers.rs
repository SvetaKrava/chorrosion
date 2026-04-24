// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};

use crate::indexers::IndexerCapabilities;
use crate::IndexerProtocol;

/// A curated template describing a well-known community indexer.
///
/// Templates are read-only presets that users can browse and use as a starting point when
/// creating a new [`chorrosion_domain::IndexerDefinition`] in their library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityIndexerTemplate {
    /// Stable slug identifier (e.g. `"prowlarr-torznab"`).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// URL template used as a preset when configuring an indexer.
    ///
    /// `{host}` is typically replaced with the user's actual host, and some built-in templates
    /// may also contain additional placeholders such as `{indexer_id}` or `{tracker_id}` that
    /// are filled in by the UI or configuration flow.
    ///
    /// Example: `"http://{host}:9696/{indexer_id}/api"`
    pub url_template: String,
    /// Protocol used by this indexer.
    pub protocol: IndexerProtocol,
    /// Whether an API key is typically required.
    pub requires_api_key: bool,
    /// Short description shown in the UI.
    pub description: String,
    /// Searchable tags (e.g. `["music", "aggregator", "usenet"]`).
    pub tags: Vec<String>,
    /// Optional default capabilities for this indexer type.
    ///
    /// `None` means no defaults are known at the template level; actual capabilities are
    /// detected at runtime when the indexer is tested or queried.
    pub default_capabilities: Option<IndexerCapabilities>,
}

/// In-memory catalog of built-in [`CommunityIndexerTemplate`]s.
///
/// The catalog is composed of owned template values stored in memory. The current
/// implementation allocates when the registry is constructed because each template stores
/// owned `String` and `Vec<String>` data.
pub struct CommunityIndexerRegistry {
    templates: Vec<CommunityIndexerTemplate>,
}

fn template(
    id: &str,
    name: &str,
    url_template: &str,
    protocol: IndexerProtocol,
    requires_api_key: bool,
    description: &str,
    tags: &[&str],
) -> CommunityIndexerTemplate {
    CommunityIndexerTemplate {
        id: id.to_string(),
        name: name.to_string(),
        url_template: url_template.to_string(),
        protocol,
        requires_api_key,
        description: description.to_string(),
        tags: tags.iter().map(|tag| tag.to_string()).collect(),
        default_capabilities: None,
    }
}

fn built_in_templates() -> Vec<CommunityIndexerTemplate> {
    vec![
        template(
            "prowlarr-torznab",
            "Prowlarr (Torznab)",
            "http://{host}:9696/{indexer_id}/api",
            IndexerProtocol::Torznab,
            true,
            "Prowlarr torrent indexer aggregator via Torznab API.",
            &["torrent", "aggregator", "torznab", "prowlarr"],
        ),
        template(
            "jackett-torznab",
            "Jackett (Torznab)",
            "http://{host}:9117/api/v2.0/indexers/{tracker_id}/results/torznab/",
            IndexerProtocol::Torznab,
            true,
            "Jackett torrent tracker aggregator via Torznab API.",
            &["torrent", "aggregator", "torznab", "jackett"],
        ),
        template(
            "nzbhydra2-newznab",
            "NZBHydra2 (Newznab)",
            "http://{host}:5076/api",
            IndexerProtocol::Newznab,
            true,
            "NZBHydra2 Usenet indexer aggregator via Newznab API.",
            &["usenet", "aggregator", "newznab", "nzbhydra2"],
        ),
        template(
            "generic-torznab",
            "Generic Torznab",
            "http://{host}/api",
            IndexerProtocol::Torznab,
            false,
            "Generic Torznab-compatible indexer endpoint.",
            &["torrent", "torznab", "generic"],
        ),
        template(
            "generic-newznab",
            "Generic Newznab",
            "http://{host}/api",
            IndexerProtocol::Newznab,
            false,
            "Generic Newznab-compatible indexer endpoint.",
            &["usenet", "newznab", "generic"],
        ),
        template(
            "redacted-gazelle",
            "Redacted (Gazelle)",
            "https://{host}",
            IndexerProtocol::Gazelle,
            true,
            "Redacted.ch music-focused private torrent tracker via Gazelle API.",
            &["torrent", "music", "gazelle", "private"],
        ),
        template(
            "orpheus-gazelle",
            "Orpheus Network (Gazelle)",
            "https://{host}",
            IndexerProtocol::Gazelle,
            true,
            "Orpheus Network music-focused private torrent tracker via Gazelle API.",
            &["torrent", "music", "gazelle", "private"],
        ),
        template(
            "nzbplanet-newznab",
            "NZBPlanet (Newznab)",
            "https://{host}/api",
            IndexerProtocol::Newznab,
            true,
            "NZBPlanet public Usenet indexer via Newznab API.",
            &["usenet", "newznab", "public"],
        ),
    ]
}

// ---------------------------------------------------------------------------
// Registry implementation
// ---------------------------------------------------------------------------

impl CommunityIndexerRegistry {
    /// Create a registry pre-populated with all built-in templates.
    pub fn built_in() -> Self {
        Self {
            templates: built_in_templates(),
        }
    }

    /// Return all templates in their display order.
    pub fn list_all(&self) -> Vec<&CommunityIndexerTemplate> {
        self.templates.iter().collect()
    }

    /// Find a template by its stable `id` slug. Returns `None` if not found.
    pub fn find_by_id(&self, id: &str) -> Option<&CommunityIndexerTemplate> {
        self.templates.iter().find(|t| t.id == id)
    }

    /// Search templates whose `name` or `description` contains `query` (case-insensitive).
    /// Returns results in display order.
    pub fn search(&self, query: &str) -> Vec<&CommunityIndexerTemplate> {
        let q = query.to_lowercase();
        self.templates
            .iter()
            .filter(|t| {
                t.name.to_lowercase().contains(&q) || t.description.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Return all templates that use the given [`IndexerProtocol`].
    pub fn by_protocol(&self, protocol: &IndexerProtocol) -> Vec<&CommunityIndexerTemplate> {
        self.templates
            .iter()
            .filter(|t| &t.protocol == protocol)
            .collect()
    }

    /// Return all templates that carry the given tag (case-insensitive).
    pub fn by_tag(&self, tag: &str) -> Vec<&CommunityIndexerTemplate> {
        let tag_lower = tag.to_lowercase();
        self.templates
            .iter()
            .filter(|t| t.tags.iter().any(|tg| tg.to_lowercase() == tag_lower))
            .collect()
    }

    /// Total number of templates in the registry.
    pub fn count(&self) -> usize {
        self.templates.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> CommunityIndexerRegistry {
        CommunityIndexerRegistry::built_in()
    }

    #[test]
    fn built_in_contains_expected_count() {
        let r = registry();
        assert_eq!(r.count(), built_in_templates().len());
        assert!(r.count() > 0, "registry must not be empty");
    }

    #[test]
    fn list_all_returns_all_templates() {
        let r = registry();
        let all = r.list_all();
        assert_eq!(all.len(), r.count());
    }

    #[test]
    fn find_by_id_returns_correct_template() {
        let r = registry();
        let t = r
            .find_by_id("prowlarr-torznab")
            .expect("prowlarr-torznab must exist");
        assert_eq!(t.id, "prowlarr-torznab");
        assert_eq!(t.protocol, IndexerProtocol::Torznab);
        assert!(t.requires_api_key);
    }

    #[test]
    fn find_by_id_returns_none_for_unknown() {
        let r = registry();
        assert!(r.find_by_id("does-not-exist").is_none());
    }

    #[test]
    fn find_by_id_unknown_is_none() {
        let r = registry();
        assert!(r.find_by_id("").is_none());
        assert!(r.find_by_id("PROWLARR-TORZNAB").is_none()); // IDs are case-sensitive
    }

    #[test]
    fn search_by_name_substring() {
        let r = registry();
        let results = r.search("prowlarr");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "prowlarr-torznab");
    }

    #[test]
    fn search_by_description_substring() {
        let r = registry();
        let results = r.search("aggregator");
        // prowlarr, nzbhydra2, jackett all mention "aggregator"
        assert!(
            results.len() >= 3,
            "expected at least 3 aggregator results, got {}",
            results.len()
        );
    }

    #[test]
    fn search_is_case_insensitive() {
        let r = registry();
        let lower = r.search("gazelle");
        let upper = r.search("GAZELLE");
        assert_eq!(lower.len(), upper.len());
        assert!(!lower.is_empty());
    }

    #[test]
    fn search_empty_query_returns_all() {
        let r = registry();
        // Every template's name/description contains the empty string
        let results = r.search("");
        assert_eq!(results.len(), r.count());
    }

    #[test]
    fn by_protocol_torznab() {
        let r = registry();
        let results = r.by_protocol(&IndexerProtocol::Torznab);
        assert!(!results.is_empty());
        for t in &results {
            assert_eq!(t.protocol, IndexerProtocol::Torznab);
        }
        // prowlarr, jackett, generic-torznab must all be present
        let ids: Vec<&str> = results.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"prowlarr-torznab"));
        assert!(ids.contains(&"jackett-torznab"));
        assert!(ids.contains(&"generic-torznab"));
    }

    #[test]
    fn by_protocol_newznab() {
        let r = registry();
        let results = r.by_protocol(&IndexerProtocol::Newznab);
        assert!(!results.is_empty());
        for t in &results {
            assert_eq!(t.protocol, IndexerProtocol::Newznab);
        }
    }

    #[test]
    fn by_protocol_gazelle() {
        let r = registry();
        let results = r.by_protocol(&IndexerProtocol::Gazelle);
        assert!(!results.is_empty());
        let ids: Vec<&str> = results.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"redacted-gazelle"));
        assert!(ids.contains(&"orpheus-gazelle"));
    }

    #[test]
    fn by_protocol_custom_returns_empty() {
        let r = registry();
        let results = r.by_protocol(&IndexerProtocol::Custom);
        assert!(results.is_empty());
    }

    #[test]
    fn by_tag_music() {
        let r = registry();
        let results = r.by_tag("music");
        assert!(!results.is_empty());
        let ids: Vec<&str> = results.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"redacted-gazelle"));
        assert!(ids.contains(&"orpheus-gazelle"));
    }

    #[test]
    fn by_tag_is_case_insensitive() {
        let r = registry();
        let lower = r.by_tag("torrent");
        let upper = r.by_tag("TORRENT");
        assert_eq!(lower.len(), upper.len());
        assert!(!lower.is_empty());
    }

    #[test]
    fn by_tag_unknown_returns_empty() {
        let r = registry();
        assert!(r.by_tag("definitely-not-a-tag").is_empty());
    }

    #[test]
    fn all_template_ids_are_unique() {
        let r = registry();
        let all = r.list_all();
        let mut ids = std::collections::HashSet::new();
        for t in all {
            assert!(ids.insert(t.id.clone()), "duplicate template id: {}", t.id);
        }
    }

    #[test]
    fn all_templates_have_non_empty_fields() {
        let r = registry();
        for t in r.list_all() {
            assert!(!t.id.is_empty(), "template id must not be empty");
            assert!(
                !t.name.is_empty(),
                "template name must not be empty (id={})",
                t.id
            );
            assert!(
                !t.url_template.is_empty(),
                "url_template must not be empty (id={})",
                t.id
            );
            assert!(
                !t.description.is_empty(),
                "description must not be empty (id={})",
                t.id
            );
        }
    }

    #[test]
    fn template_url_templates_contain_placeholder() {
        let r = registry();
        for t in r.list_all() {
            assert!(
                t.url_template.contains("{host}"),
                "url_template for '{}' must contain {{host}}",
                t.id
            );
        }
    }

    #[test]
    fn clone_and_serialize_roundtrip() {
        let r = registry();
        let t = r.find_by_id("nzbhydra2-newznab").expect("must exist");
        let cloned = t.clone();
        assert_eq!(t, &cloned);
        let json = serde_json::to_string(&cloned).expect("serialize");
        let back: CommunityIndexerTemplate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, cloned);
    }
}
