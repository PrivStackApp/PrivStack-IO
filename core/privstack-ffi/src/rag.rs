//! FFI bindings for the RAG (Retrieval-Augmented Generation) vector index.
//!
//! Provides upsert, search, delete, and hash-retrieval for semantic search vectors.

use crate::{lock_handle, SdkResponse};
use serde::Deserialize;
use std::ffi::{c_char, CStr, CString};

#[derive(Deserialize)]
struct RagUpsertRequest {
    entity_id: String,
    chunk_path: String,
    plugin_id: String,
    entity_type: String,
    content_hash: String,
    dim: i32,
    embedding: Vec<f64>,
    title: String,
    link_type: String,
    indexed_at: i64,
}

#[derive(Deserialize)]
struct RagSearchRequest {
    embedding: Vec<f64>,
    limit: Option<usize>,
    entity_types: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RagDeleteRequest {
    entity_id: String,
}

#[derive(Deserialize)]
struct RagGetHashesRequest {
    entity_types: Option<Vec<String>>,
}

/// Upsert a RAG vector entry.
///
/// # Safety
/// `json` must be a valid null-terminated UTF-8 JSON string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_rag_upsert(json: *const c_char) -> *mut c_char {
    unsafe {
        let response = rag_upsert_inner(json);
        let json_out = serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"success":false,"error_code":"json_error","error_message":"Failed to serialize response"}"#.to_string()
        });
        CString::new(json_out).unwrap_or_default().into_raw()
    }
}

unsafe fn rag_upsert_inner(json: *const c_char) -> SdkResponse {
    if json.is_null() {
        return SdkResponse::err("null_pointer", "JSON is null");
    }
    let json_str = match unsafe { CStr::from_ptr(json) }.to_str() {
        Ok(s) => s,
        Err(_) => return SdkResponse::err("invalid_utf8", "JSON is not valid UTF-8"),
    };
    let req: RagUpsertRequest = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(e) => return SdkResponse::err("json_parse_error", &format!("Invalid JSON: {e}")),
    };

    let handle = lock_handle();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return SdkResponse::err("not_initialized", "PrivStack runtime not initialized"),
    };

    match handle.entity_store.rag_upsert(
        &req.entity_id,
        &req.chunk_path,
        &req.plugin_id,
        &req.entity_type,
        &req.content_hash,
        req.dim,
        &req.embedding,
        &req.title,
        &req.link_type,
        req.indexed_at,
    ) {
        Ok(()) => SdkResponse::ok_empty(),
        Err(e) => SdkResponse::err("storage_error", &format!("RAG upsert failed: {e}")),
    }
}

/// Search RAG vectors by cosine similarity.
///
/// # Safety
/// `json` must be a valid null-terminated UTF-8 JSON string.
/// The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_rag_search(json: *const c_char) -> *mut c_char {
    unsafe {
        let response = rag_search_inner(json);
        let json_out = serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"success":false,"error_code":"json_error","error_message":"Failed to serialize response"}"#.to_string()
        });
        CString::new(json_out).unwrap_or_default().into_raw()
    }
}

unsafe fn rag_search_inner(json: *const c_char) -> SdkResponse {
    if json.is_null() {
        return SdkResponse::err("null_pointer", "JSON is null");
    }
    let json_str = match unsafe { CStr::from_ptr(json) }.to_str() {
        Ok(s) => s,
        Err(_) => return SdkResponse::err("invalid_utf8", "JSON is not valid UTF-8"),
    };
    let req: RagSearchRequest = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(e) => return SdkResponse::err("json_parse_error", &format!("Invalid JSON: {e}")),
    };

    let handle = lock_handle();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return SdkResponse::err("not_initialized", "PrivStack runtime not initialized"),
    };

    let limit = req.limit.unwrap_or(20);
    let types_refs: Option<Vec<&str>> = req
        .entity_types
        .as_ref()
        .map(|v| v.iter().map(|s| s.as_str()).collect());

    match handle
        .entity_store
        .rag_search(&req.embedding, limit, types_refs.as_deref())
    {
        Ok(results) => SdkResponse::ok(serde_json::Value::Array(results)),
        Err(e) => SdkResponse::err("storage_error", &format!("RAG search failed: {e}")),
    }
}

/// Delete all RAG vectors for an entity.
///
/// # Safety
/// `json` must be a valid null-terminated UTF-8 JSON string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_rag_delete(json: *const c_char) -> *mut c_char {
    unsafe {
        let response = rag_delete_inner(json);
        let json_out = serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"success":false,"error_code":"json_error","error_message":"Failed to serialize response"}"#.to_string()
        });
        CString::new(json_out).unwrap_or_default().into_raw()
    }
}

unsafe fn rag_delete_inner(json: *const c_char) -> SdkResponse {
    if json.is_null() {
        return SdkResponse::err("null_pointer", "JSON is null");
    }
    let json_str = match unsafe { CStr::from_ptr(json) }.to_str() {
        Ok(s) => s,
        Err(_) => return SdkResponse::err("invalid_utf8", "JSON is not valid UTF-8"),
    };
    let req: RagDeleteRequest = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(e) => return SdkResponse::err("json_parse_error", &format!("Invalid JSON: {e}")),
    };

    let handle = lock_handle();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return SdkResponse::err("not_initialized", "PrivStack runtime not initialized"),
    };

    match handle.entity_store.rag_delete(&req.entity_id) {
        Ok(()) => SdkResponse::ok_empty(),
        Err(e) => SdkResponse::err("storage_error", &format!("RAG delete failed: {e}")),
    }
}

/// Get content hashes for RAG vectors (for incremental indexing skip).
///
/// # Safety
/// `json` must be a valid null-terminated UTF-8 JSON string.
/// The returned pointer must be freed with `privstack_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn privstack_rag_get_hashes(json: *const c_char) -> *mut c_char {
    unsafe {
        let response = rag_get_hashes_inner(json);
        let json_out = serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"success":false,"error_code":"json_error","error_message":"Failed to serialize response"}"#.to_string()
        });
        CString::new(json_out).unwrap_or_default().into_raw()
    }
}

unsafe fn rag_get_hashes_inner(json: *const c_char) -> SdkResponse {
    if json.is_null() {
        return SdkResponse::err("null_pointer", "JSON is null");
    }
    let json_str = match unsafe { CStr::from_ptr(json) }.to_str() {
        Ok(s) => s,
        Err(_) => return SdkResponse::err("invalid_utf8", "JSON is not valid UTF-8"),
    };
    let req: RagGetHashesRequest = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(e) => return SdkResponse::err("json_parse_error", &format!("Invalid JSON: {e}")),
    };

    let handle = lock_handle();
    let handle = match handle.as_ref() {
        Some(h) => h,
        None => return SdkResponse::err("not_initialized", "PrivStack runtime not initialized"),
    };

    let types_refs: Option<Vec<&str>> = req
        .entity_types
        .as_ref()
        .map(|v| v.iter().map(|s| s.as_str()).collect());

    match handle
        .entity_store
        .rag_get_hashes(types_refs.as_deref())
    {
        Ok(hashes) => {
            let arr: Vec<serde_json::Value> = hashes
                .into_iter()
                .map(|(eid, cp, ch)| {
                    serde_json::json!({
                        "entity_id": eid,
                        "chunk_path": cp,
                        "content_hash": ch,
                    })
                })
                .collect();
            SdkResponse::ok(serde_json::Value::Array(arr))
        }
        Err(e) => SdkResponse::err("storage_error", &format!("RAG get hashes failed: {e}")),
    }
}
