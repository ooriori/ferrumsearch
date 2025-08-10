use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// ==================== CORE DATA STRUCTURES ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub content: String,
    pub metadata: HashMap<String, String>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub content: String,
    pub score: f32,
    pub highlights: Vec<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total_hits: usize,
    pub query_time_ms: u64,
    pub page: usize,
    pub per_page: usize,
    pub total_pages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_documents: usize,
    pub index_size_mb: f64,
    pub last_updated: u64,
    pub version: String,
}

// ==================== SEARCH QUERY STRUCTURE ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub fuzzy: bool,
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub filters: Option<HashMap<String, String>>,
    pub sort_by: Option<String>,
    pub highlight: bool,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            fuzzy: false,
            page: Some(1),
            per_page: Some(10),
            filters: None,
            sort_by: None,
            highlight: true,
        }
    }
}

// ==================== SEARCH ENGINE CORE ====================

pub struct FerrumSearch {
    documents: Arc<RwLock<HashMap<String, Document>>>,
    inverted_index: Arc<RwLock<HashMap<String, Vec<String>>>>,
    word_frequencies: Arc<RwLock<HashMap<String, HashMap<String, f32>>>>,
    document_lengths: Arc<RwLock<HashMap<String, usize>>>,
    total_documents: Arc<RwLock<usize>>,
}

impl FerrumSearch {
    pub fn new() -> Self {
        Self {
            documents: Arc::new(RwLock::new(HashMap::new())),
            inverted_index: Arc::new(RwLock::new(HashMap::new())),
            word_frequencies: Arc::new(RwLock::new(HashMap::new())),
            document_lengths: Arc::new(RwLock::new(HashMap::new())),
            total_documents: Arc::new(RwLock::new(0)),
        }
    }

    // ==================== INDEXING OPERATIONS ====================

    pub fn add_document(&self, mut document: Document) -> Result<(), String> {
        if document.id.is_empty() {
            document.id = Uuid::new_v4().to_string();
        }

        let doc_id = document.id.clone();
        let text = format!("{} {}", document.title, document.content);
        let tokens = self.tokenize(&text);
        
        // Store document
        {
            let mut docs = self.documents.write().unwrap();
            let is_new = !docs.contains_key(&doc_id);
            docs.insert(doc_id.clone(), document);
            
            if is_new {
                let mut total = self.total_documents.write().unwrap();
                *total += 1;
            }
        }

        // Update inverted index and frequencies
        {
            let mut index = self.inverted_index.write().unwrap();
            let mut frequencies = self.word_frequencies.write().unwrap();
            let mut doc_lengths = self.document_lengths.write().unwrap();

            // Remove old entries if updating
            self.remove_document_from_index(&doc_id, &mut index, &mut frequencies);

            // Add new entries
            let mut word_count = HashMap::new();
            for token in &tokens {
                *word_count.entry(token.clone()).or_insert(0) += 1;
                
                index.entry(token.clone())
                    .or_insert_with(Vec::new)
                    .push(doc_id.clone());
            }

            // Calculate TF scores
            let doc_length = tokens.len();
            doc_lengths.insert(doc_id.clone(), doc_length);
            
            let mut doc_frequencies = HashMap::new();
            for (word, count) in word_count {
                let tf = count as f32 / doc_length as f32;
                doc_frequencies.insert(word, tf);
            }
            frequencies.insert(doc_id, doc_frequencies);
        }

        Ok(())
    }

    pub fn remove_document(&self, doc_id: &str) -> Result<(), String> {
        {
            let mut docs = self.documents.write().unwrap();
            if docs.remove(doc_id).is_some() {
                let mut total = self.total_documents.write().unwrap();
                *total = total.saturating_sub(1);
            }
        }

        {
            let mut index = self.inverted_index.write().unwrap();
            let mut frequencies = self.word_frequencies.write().unwrap();
            self.remove_document_from_index(doc_id, &mut index, &mut frequencies);
        }

        Ok(())
    }

    fn remove_document_from_index(
        &self,
        doc_id: &str,
        index: &mut HashMap<String, Vec<String>>,
        frequencies: &mut HashMap<String, HashMap<String, f32>>,
    ) {
        frequencies.remove(doc_id);
        
        // Remove from inverted index
        let words_to_clean: Vec<String> = index
            .iter()
            .filter(|(_, docs)| docs.contains(&doc_id.to_string()))
            .map(|(word, _)| word.clone())
            .collect();

        for word in words_to_clean {
            if let Some(docs) = index.get_mut(&word) {
                docs.retain(|id| id != doc_id);
                if docs.is_empty() {
                    index.remove(&word);
                }
            }
        }
    }

    // ==================== SEARCH OPERATIONS ====================

    pub fn search(&self, query: SearchQuery) -> Result<SearchResponse, String> {
        let start_time = SystemTime::now();
        
        let tokens = self.tokenize(&query.query);
        if tokens.is_empty() {
            return Ok(SearchResponse {
                results: vec![],
                total_hits: 0,
                query_time_ms: 0,
                page: query.page.unwrap_or(1),
                per_page: query.per_page.unwrap_or(10),
                total_pages: 0,
            });
        }

        let mut scores = HashMap::new();
        let docs = self.documents.read().unwrap();
        let index = self.inverted_index.read().unwrap();
        let frequencies = self.word_frequencies.read().unwrap();
        let total_docs = *self.total_documents.read().unwrap();

        // Calculate BM25 scores
        for token in &tokens {
            let matching_docs = if query.fuzzy {
                self.fuzzy_search_token(&token, &index)
            } else {
                index.get(token).cloned().unwrap_or_default()
            };

            let df = matching_docs.len();
            if df == 0 { continue; }

            let idf = ((total_docs as f32 - df as f32 + 0.5) / (df as f32 + 0.5)).ln();

            for doc_id in matching_docs {
                if let Some(doc_freqs) = frequencies.get(&doc_id) {
                    if let Some(&tf) = doc_freqs.get(token) {
                        let k1 = 1.5;
                        let b = 0.75;
                        let doc_len = self.document_lengths.read().unwrap()
                            .get(&doc_id).copied().unwrap_or(1);
                        let avg_doc_len = 100.0; // Simplified average

                        let bm25_tf = (tf * (k1 + 1.0)) / 
                            (tf + k1 * (1.0 - b + b * (doc_len as f32 / avg_doc_len)));
                        
                        let score = idf * bm25_tf;
                        *scores.entry(doc_id.clone()).or_insert(0.0) += score;
                    }
                }
            }
        }

        // Apply filters
        if let Some(filters) = &query.filters {
            scores.retain(|doc_id, _| {
                if let Some(doc) = docs.get(doc_id) {
                    filters.iter().all(|(key, value)| {
                        doc.metadata.get(key).map_or(false, |v| v == value)
                    })
                } else {
                    false
                }
            });
        }

        // Sort results
        let mut sorted_results: Vec<_> = scores.into_iter().collect();
        sorted_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let total_hits = sorted_results.len();
        let page = query.page.unwrap_or(1);
        let per_page = query.per_page.unwrap_or(10);
        let total_pages = (total_hits + per_page - 1) / per_page;

        // Pagination
        let start = (page - 1) * per_page;
        let end = std::cmp::min(start + per_page, total_hits);
        
        let mut results = Vec::new();
        for (doc_id, score) in sorted_results.iter().skip(start).take(end - start) {
            if let Some(doc) = docs.get(doc_id) {
                let highlights = if query.highlight {
                    self.generate_highlights(doc, &tokens)
                } else {
                    vec![]
                };

                results.push(SearchResult {
                    id: doc.id.clone(),
                    title: doc.title.clone(),
                    content: self.truncate_content(&doc.content, 200),
                    score: *score,
                    highlights,
                    metadata: doc.metadata.clone(),
                });
            }
        }

        let query_time_ms = start_time.elapsed()
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Ok(SearchResponse {
            results,
            total_hits,
            query_time_ms,
            page,
            per_page,
            total_pages,
        })
    }

    // ==================== AUTOCOMPLETE & SUGGESTIONS ====================

    pub fn autocomplete(&self, prefix: &str, limit: usize) -> Vec<String> {
        let index = self.inverted_index.read().unwrap();
        let mut suggestions: Vec<_> = index
            .keys()
            .filter(|word| word.starts_with(&prefix.to_lowercase()))
            .take(limit)
            .cloned()
            .collect();
        
        suggestions.sort();
        suggestions
    }

    pub fn suggest(&self, query: &str) -> Vec<String> {
        let tokens = self.tokenize(query);
        let index = self.inverted_index.read().unwrap();
        
        let mut suggestions = Vec::new();
        for token in tokens {
            let fuzzy_matches = self.fuzzy_search_token(&token, &index);
            for doc_id in fuzzy_matches.iter().take(3) {
                if let Some(doc) = self.documents.read().unwrap().get(doc_id) {
                    suggestions.push(doc.title.clone());
                }
            }
        }
        
        suggestions.truncate(5);
        suggestions
    }

    // ==================== UTILITY METHODS ====================

    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .filter(|word| word.len() > 2)
            .map(|s| s.to_string())
            .collect()
    }

    fn fuzzy_search_token(&self, token: &str, index: &HashMap<String, Vec<String>>) -> Vec<String> {
        let mut matches = Vec::new();
        
        // Exact match first
        if let Some(docs) = index.get(token) {
            matches.extend_from_slice(docs);
        }

        // Fuzzy matches (edit distance = 1)
        for word in index.keys() {
            if word != token && self.edit_distance(token, word) <= 1 {
                if let Some(docs) = index.get(word) {
                    matches.extend_from_slice(docs);
                }
            }
        }

        matches.sort();
        matches.dedup();
        matches
    }

    fn edit_distance(&self, a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let mut dp = vec![vec![0; b_chars.len() + 1]; a_chars.len() + 1];

        for i in 0..=a_chars.len() {
            dp[i][0] = i;
        }
        for j in 0..=b_chars.len() {
            dp[0][j] = j;
        }

        for i in 1..=a_chars.len() {
            for j in 1..=b_chars.len() {
                let cost = if a_chars[i-1] == b_chars[j-1] { 0 } else { 1 };
                dp[i][j] = std::cmp::min(
                    std::cmp::min(dp[i-1][j] + 1, dp[i][j-1] + 1),
                    dp[i-1][j-1] + cost
                );
            }
        }

        dp[a_chars.len()][b_chars.len()]
    }

    fn generate_highlights(&self, doc: &Document, tokens: &[String]) -> Vec<String> {
        let full_text = format!("{} {}", doc.title, doc.content);
        let mut highlights = Vec::new();
        
        for token in tokens {
            if let Some(start) = full_text.to_lowercase().find(&token.to_lowercase()) {
                let context_start = start.saturating_sub(50);
                let context_end = std::cmp::min(start + token.len() + 50, full_text.len());
                
                let mut highlight = full_text[context_start..context_end].to_string();
                if context_start > 0 {
                    highlight = format!("...{}", highlight);
                }
                if context_end < full_text.len() {
                    highlight = format!("{}...", highlight);
                }
                
                highlights.push(highlight);
            }
        }
        
        highlights.truncate(3);
        highlights
    }

    fn truncate_content(&self, content: &str, max_len: usize) -> String {
        if content.len() <= max_len {
            content.to_string()
        } else {
            format!("{}...", &content[..max_len])
        }
    }

    // ==================== STATS & MONITORING ====================

    pub fn get_stats(&self) -> IndexStats {
        let total_docs = *self.total_documents.read().unwrap();
        let estimated_size = total_docs * 1024; // Rough estimation
        
        IndexStats {
            total_documents: total_docs,
            index_size_mb: estimated_size as f64 / 1024.0 / 1024.0,
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            version: "1.0.0".to_string(),
        }
    }

    pub fn bulk_import(&self, documents: Vec<Document>) -> Result<usize, String> {
        let mut success_count = 0;
        
        for doc in documents {
            match self.add_document(doc) {
                Ok(_) => success_count += 1,
                Err(e) => eprintln!("Failed to import document: {}", e),
            }
        }
        
        Ok(success_count)
    }

    pub fn clear_index(&self) -> Result<(), String> {
        *self.documents.write().unwrap() = HashMap::new();
        *self.inverted_index.write().unwrap() = HashMap::new();
        *self.word_frequencies.write().unwrap() = HashMap::new();
        *self.document_lengths.write().unwrap() = HashMap::new();
        *self.total_documents.write().unwrap() = 0;
        Ok(())
    }
}

// ==================== DEMO & TESTING ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_search() {
        let engine = FerrumSearch::new();
        
        let doc1 = Document {
            id: "1".to_string(),
            title: "Rust Programming".to_string(),
            content: "Rust is a systems programming language focused on safety and performance".to_string(),
            metadata: HashMap::new(),
            timestamp: 0,
        };

        let doc2 = Document {
            id: "2".to_string(),
            title: "Web Development".to_string(),
            content: "Building web applications with modern frameworks and tools".to_string(),
            metadata: HashMap::new(),
            timestamp: 0,
        };

        engine.add_document(doc1).unwrap();
        engine.add_document(doc2).unwrap();

        let query = SearchQuery {
            query: "rust programming".to_string(),
            ..Default::default()
        };

        let results = engine.search(query).unwrap();
        assert_eq!(results.total_hits, 1);
        assert_eq!(results.results[0].id, "1");
    }

    #[test]
    fn test_fuzzy_search() {
        let engine = FerrumSearch::new();
        
        let doc = Document {
            id: "1".to_string(),
            title: "Programming".to_string(),
            content: "Advanced programming concepts".to_string(),
            metadata: HashMap::new(),
            timestamp: 0,
        };

        engine.add_document(doc).unwrap();

        let query = SearchQuery {
            query: "programing".to_string(), // Typo
            fuzzy: true,
            ..Default::default()
        };

        let results = engine.search(query).unwrap();
        assert_eq!(results.total_hits, 1);
    }
}

fn main() {
    println!("🔍 FerrumSearch - High-Performance Search Engine");
    println!("================================================");
    
    let engine = FerrumSearch::new();
    
    // Demo data
    let demo_docs = vec![
        Document {
            id: "rust-guide".to_string(),
            title: "The Rust Programming Language Guide".to_string(),
            content: "Rust is a systems programming language that runs blazingly fast, prevents segfaults, and guarantees thread safety. It accomplishes these goals by being memory safe without using garbage collection.".to_string(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("category".to_string(), "programming".to_string());
                meta.insert("difficulty".to_string(), "intermediate".to_string());
                meta
            },
            timestamp: 1640995200,
        },
        Document {
            id: "web-dev-trends".to_string(),
            title: "Modern Web Development Trends 2024".to_string(),
            content: "Web development continues to evolve with new frameworks, tools, and best practices. React, Vue, and Angular dominate the frontend landscape while Node.js powers many backend applications.".to_string(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("category".to_string(), "web".to_string());
                meta.insert("year".to_string(), "2024".to_string());
                meta
            },
            timestamp: 1704067200,
        },
        Document {
            id: "search-algorithms".to_string(),
            title: "Understanding Search Algorithms".to_string(),
            content: "Search algorithms are fundamental to computer science. From simple linear search to complex full-text search engines, understanding how search works is crucial for building efficient applications.".to_string(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("category".to_string(), "algorithms".to_string());
                meta.insert("difficulty".to_string(), "advanced".to_string());
                meta
            },
            timestamp: 1672531200,
        },
    ];

    // Import demo data
    match engine.bulk_import(demo_docs) {
        Ok(count) => println!("✅ Successfully imported {} documents", count),
        Err(e) => println!("❌ Import failed: {}", e),
    }

    // Demo searches
    println!("\n🔍 Demo Searches:");
    println!("=================");

    // Basic search
    let query = SearchQuery {
        query: "rust programming".to_string(),
        ..Default::default()
    };
    
    match engine.search(query) {
        Ok(response) => {
            println!("\n📊 Query: 'rust programming' ({}ms)", response.query_time_ms);
            println!("   Results: {}/{}", response.results.len(), response.total_hits);
            for result in &response.results {
                println!("   • {} (score: {:.2})", result.title, result.score);
            }
        },
        Err(e) => println!("❌ Search failed: {}", e),
    }

    // Fuzzy search
    let fuzzy_query = SearchQuery {
        query: "algoritms".to_string(), // Typo intentional
        fuzzy: true,
        ..Default::default()
    };
    
    match engine.search(fuzzy_query) {
        Ok(response) => {
            println!("\n📊 Fuzzy Query: 'algoritms' ({}ms)", response.query_time_ms);
            println!("   Results: {}/{}", response.results.len(), response.total_hits);
            for result in &response.results {
                println!("   • {} (score: {:.2})", result.title, result.score);
            }
        },
        Err(e) => println!("❌ Fuzzy search failed: {}", e),
    }

    // Filtered search
    let filtered_query = SearchQuery {
        query: "development".to_string(),
        filters: Some({
            let mut filters = HashMap::new();
            filters.insert("category".to_string(), "web".to_string());
            filters
        }),
        ..Default::default()
    };
    
    match engine.search(filtered_query) {
        Ok(response) => {
            println!("\n📊 Filtered Query: 'development' + category:web ({}ms)", response.query_time_ms);
            println!("   Results: {}/{}", response.results.len(), response.total_hits);
            for result in &response.results {
                println!("   • {} (score: {:.2})", result.title, result.score);
            }
        },
        Err(e) => println!("❌ Filtered search failed: {}", e),
    }

    // Autocomplete demo
    println!("\n🔤 Autocomplete for 'prog':");
    let suggestions = engine.autocomplete("prog", 5);
    for suggestion in suggestions {
        println!("   • {}", suggestion);
    }

    // Stats
    let stats = engine.get_stats();
    println!("\n📈 Index Statistics:");
    println!("   Documents: {}", stats.total_documents);
    println!("   Index Size: {:.2} MB", stats.index_size_mb);
    println!("   Version: {}", stats.version);
    
    println!("\n🚀 FerrumSearch is ready for production!");
}