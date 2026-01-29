//! JLCPCB API client for component search (optional feature)
//!
//! IMPORTANT: JLCPCB API requires manual application approval.
//! See: https://jlcpcb.com/help/article/jlcpcb-online-api-available-now
//!
//! This module is only compiled when the `jlcpcb` feature is enabled.

use crate::error::LibraryError;
use crate::models::{Component, ComponentId, ComponentMetadata, LibraryInfo};
use crate::sources::LibrarySource;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// JLCPCB API client (optional, requires API key)
pub struct JLCPCBSource {
    client: Client,
    api_key: String,
    base_url: String,
}

impl JLCPCBSource {
    /// Create a new JLCPCB source with API key
    ///
    /// # Arguments
    /// * `api_key` - JLCPCB API key (requires application approval)
    ///
    /// # Note
    /// Default base URL is https://api.jlcpcb.com
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: "https://api.jlcpcb.com".to_string(),
        }
    }

    /// Search JLCPCB parts catalog
    ///
    /// # Arguments
    /// * `query` - Search keyword (component name, part number, etc.)
    /// * `page` - Page number (1-indexed)
    /// * `page_size` - Number of results per page
    ///
    /// # Returns
    /// Vector of components matching the search query
    ///
    /// # Errors
    /// Returns `LibraryError::ApiError` on HTTP errors or network failures
    pub async fn search_api(
        &self,
        query: &str,
        page: usize,
        page_size: usize,
    ) -> Result<Vec<Component>, LibraryError> {
        let response = self
            .client
            .get(format!("{}/components/search", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .query(&[
                ("keyword", query),
                ("page", &page.to_string()),
                ("pageSize", &page_size.to_string()),
            ])
            .send()
            .await
            .map_err(|e| {
                LibraryError::ApiError(format!("JLCPCB API request failed: {}", e))
            })?;

        // Check for HTTP errors
        if !response.status().is_success() {
            return Err(LibraryError::ApiError(format!(
                "JLCPCB API returned status {}: {}",
                response.status(),
                response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unable to read response".to_string())
            )));
        }

        // Parse JSON response
        let jlcpcb_response: JLCPCBResponse = response.json().await.map_err(|e| {
            LibraryError::ApiError(format!("Failed to parse JLCPCB response: {}", e))
        })?;

        // Convert to Component structs
        let components: Vec<Component> = jlcpcb_response
            .data
            .into_iter()
            .map(|jlc_comp| jlc_comp.into_component())
            .collect();

        Ok(components)
    }
}

impl LibrarySource for JLCPCBSource {
    fn source_name(&self) -> &str {
        "jlcpcb"
    }

    fn list_libraries(&self) -> Result<Vec<LibraryInfo>, LibraryError> {
        // JLCPCB is one massive catalog, not multiple libraries
        Ok(vec![LibraryInfo {
            source: "jlcpcb".to_string(),
            name: "JLCPCB Parts Catalog".to_string(),
            path: None,
            version: Some(chrono::Utc::now().to_rfc3339()),
            enabled: true,
            component_count: 0, // Unknown size
        }])
    }

    fn import_library(&self, _name: &str) -> Result<Vec<Component>, LibraryError> {
        // Full catalog import is impractical (millions of parts)
        Err(LibraryError::NotSupported(
            "JLCPCB catalog is too large for full import. Use search_api() instead.".to_string(),
        ))
    }
}

/// JLCPCB API response structure
#[derive(Debug, Deserialize)]
struct JLCPCBResponse {
    data: Vec<JLCPCBComponent>,
    #[allow(dead_code)]
    total: usize,
}

/// JLCPCB component from API
#[derive(Debug, Deserialize, Serialize)]
struct JLCPCBComponent {
    #[serde(rename = "componentCode")]
    component_code: String,

    #[serde(rename = "componentName")]
    component_name: String,

    #[serde(rename = "componentDesignator", default)]
    designator: Option<String>,

    #[serde(rename = "componentModel", default)]
    model: Option<String>,

    #[serde(rename = "stockCount", default)]
    stock_count: Option<u32>,

    #[serde(default)]
    price: Option<String>,

    #[serde(default)]
    describe: Option<String>,

    #[serde(rename = "manufacturerName", default)]
    manufacturer: Option<String>,

    #[serde(rename = "componentSpecificationEn", default)]
    specification: Option<String>,
}

impl JLCPCBComponent {
    /// Convert JLCPCB component to internal Component model
    fn into_component(self) -> Component {
        Component {
            id: ComponentId::new("jlcpcb", &self.component_code),
            library: "JLCPCB Parts Catalog".to_string(),
            category: self.designator.clone(), // Use designator as category (R, C, U, etc.)
            footprint_data: None,               // JLCPCB API doesn't provide footprint data
            metadata: ComponentMetadata {
                description: self.describe.or(Some(self.component_name.clone())),
                datasheet_url: None, // Not provided by API
                manufacturer: self.manufacturer,
                mpn: Some(self.component_code.clone()),
                value: self.specification,
                package: self.model,
                step_model_path: None, // Not provided by API
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jlcpcb_source_name() {
        let source = JLCPCBSource::new("test-key".to_string());
        assert_eq!(source.source_name(), "jlcpcb");
    }

    #[test]
    fn test_list_libraries() {
        let source = JLCPCBSource::new("test-key".to_string());
        let libraries = source.list_libraries().unwrap();
        assert_eq!(libraries.len(), 1);
        assert_eq!(libraries[0].source, "jlcpcb");
        assert_eq!(libraries[0].name, "JLCPCB Parts Catalog");
    }

    #[test]
    fn test_import_library_not_supported() {
        let source = JLCPCBSource::new("test-key".to_string());
        let result = source.import_library("JLCPCB Parts Catalog");
        assert!(result.is_err());
        match result {
            Err(LibraryError::NotSupported(msg)) => {
                assert!(msg.contains("too large for full import"));
            }
            _ => panic!("Expected NotSupported error"),
        }
    }

    #[test]
    fn test_component_conversion() {
        let jlc_comp = JLCPCBComponent {
            component_code: "C12345".to_string(),
            component_name: "10K Resistor".to_string(),
            designator: Some("R".to_string()),
            model: Some("0805".to_string()),
            stock_count: Some(10000),
            price: Some("$0.01".to_string()),
            describe: Some("10K ohm resistor".to_string()),
            manufacturer: Some("Yageo".to_string()),
            specification: Some("10k".to_string()),
        };

        let component = jlc_comp.into_component();
        assert_eq!(component.id.source, "jlcpcb");
        assert_eq!(component.id.name, "C12345");
        assert_eq!(component.category, Some("R".to_string()));
        assert_eq!(component.metadata.manufacturer, Some("Yageo".to_string()));
        assert_eq!(component.metadata.value, Some("10k".to_string()));
        assert_eq!(component.metadata.package, Some("0805".to_string()));
    }
}
