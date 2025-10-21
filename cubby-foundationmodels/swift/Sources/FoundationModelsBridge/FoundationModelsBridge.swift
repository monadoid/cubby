import Foundation
import FoundationModels
import SwiftRs

// Define schemas that conform to Generable for structured output

@Generable
struct PersonInfo: Codable {
    var name: String
    var age: Int
}

@Generable
struct ArticleSummary: Codable {
    var title: String
    var summary: String
    var key_points: [String]
}

/// Check availability of the default system language model
///
/// Returns JSON string with fields:
/// - status: "available" | "unavailable" | "unknown"
/// - reason: optional localized description for unavailable state
/// - reason_code: optional debug identifier for unavailable state
@_cdecl("fm_check_model_availability")
public func fm_check_model_availability() -> SRString {
    let model = SystemLanguageModel()
    
    enum AvailabilityStatus: String {
        case available
        case unavailable
        case unknown
    }
    
    var payload: [String: Any] = [:]
    
    switch model.availability {
    case .available:
        payload["status"] = AvailabilityStatus.available.rawValue
    case .unavailable(let reason):
        payload["status"] = AvailabilityStatus.unavailable.rawValue
        let reasonDescription = String(describing: reason)
        payload["reason"] = reasonDescription
        payload["reason_code"] = String(reflecting: reason)
    @unknown default:
        payload["status"] = AvailabilityStatus.unknown.rawValue
    }
    
    if let jsonData = try? JSONSerialization.data(withJSONObject: payload),
       let jsonString = String(data: jsonData, encoding: .utf8) {
        return SRString(jsonString)
    }
    
    // Fallback JSON on serialization failure
    return SRString("{\"status\":\"unknown\",\"reason\":\"failed to encode availability\"}")
}

/// Synchronous wrapper for FoundationModels async API
/// 
/// **WARNING: This function blocks the calling thread** until the async operation completes.
/// Only call this from:
/// - Background threads
/// - tokio::task::spawn_blocking contexts in Rust
/// - Never from UI/main thread
///
/// Returns JSON string on success, or error JSON {"error": "..."} on failure
@_cdecl("fm_generate_person")
public func fm_generate_person(prompt: SRString) -> SRString {
    let promptString = prompt.toString()
    
    // Use async let with runBlocking pattern
    let result: Result<String, Error> = runAsyncAndWait {
        do {
            let jsonString = try await generatePersonAsync(prompt: promptString)
            return .success(jsonString)
        } catch {
            return .failure(error)
        }
    }
    
    switch result {
    case .success(let json):
        return SRString(json)
    case .failure(let error):
        // Return error as JSON
        let errorDict = ["error": error.localizedDescription]
        if let errorData = try? JSONEncoder().encode(errorDict),
           let errorJson = String(data: errorData, encoding: .utf8) {
            return SRString(errorJson)
        }
        return SRString("{\"error\":\"unknown error\"}")
    }
}

/// Helper to run async code synchronously
/// 
/// This uses a semaphore to block the calling thread until the async operation completes.
/// This is necessary because the exported C functions must be synchronous for Rust FFI entry points.
/// 
/// The Sendable constraint ensures thread-safety across the async boundary.
func runAsyncAndWait<T: Sendable>(_ operation: @Sendable @escaping () async -> T) -> T {
    let semaphore = DispatchSemaphore(value: 0)
    var result: T!
    
    Task {
        result = await operation()
        semaphore.signal()
    }
    
    semaphore.wait()
    return result
}

// Async implementation that calls FoundationModels
private func generatePersonAsync(prompt: String) async throws -> String {
    // Create a system language model
    let model = SystemLanguageModel()
    
    // Initialize a session with the model
    let session = LanguageModelSession(model: model, tools: [], instructions: nil)
    
    // Create a prompt from the input string
    let promptObj = Prompt(prompt)
    
    // Generate structured output using the Generable protocol
    let options = GenerationOptions()
    let response = try await session.respond(
        to: promptObj,
        generating: PersonInfo.self,
        includeSchemaInPrompt: false,
        options: options
    )
    
    // Extract the generated PersonInfo
    let personInfo = response.content
    
    // Convert to JSON string
    let encoder = JSONEncoder()
    encoder.outputFormatting = .prettyPrinted
    
    let jsonData = try encoder.encode(personInfo)
    guard let jsonString = String(data: jsonData, encoding: .utf8) else {
        throw NSError(domain: "FoundationModelsBridge", code: -2, userInfo: [NSLocalizedDescriptionKey: "failed to convert json data to string"])
    }
    
    return jsonString
}

// MARK: - Streaming Support

/// Callback type for streaming responses
/// Parameters: context, content_json, raw_content
/// Completion is signaled by passing nil for both json pointers
typealias StreamCallback = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?, UnsafePointer<CChar>?) -> Void

/// Streaming wrapper for FoundationModels async API
///
/// **WARNING: This function blocks the calling thread** until streaming completes.
/// Calls the callback for each partial snapshot as it arrives.
///
/// Callback parameters:
/// - context: user-provided context pointer
/// - content_json: JSON string of partial PersonInfo (nil signals completion/error)
/// - raw_content: raw text generated so far (nil for error, empty for completion)
@_cdecl("fm_generate_person_stream_sync")
func fm_generate_person_stream(
    prompt: UnsafePointer<CChar>,
    callback: StreamCallback,
    context: UnsafeMutableRawPointer?
) {
    let promptString = String(cString: prompt)
    
    // Convert pointer to Int for Sendable compliance, then back to pointer in async context
    let contextInt = Int(bitPattern: context)
    
    let result: Result<Void, Error> = runAsyncAndWait {
        do {
            let contextPtr = UnsafeMutableRawPointer(bitPattern: contextInt)
            try await generatePersonStreamAsync(
                prompt: promptString,
                callback: callback,
                context: contextPtr
            )
            return .success(())
        } catch {
            return .failure(error)
        }
    }
    
    // Signal completion or error
    switch result {
    case .success:
        // Signal completion with nil content_json and empty raw_content
        callback(context, nil, "".withCString { $0 })
    case .failure(let error):
        // Signal error with error json and nil raw_content
        let errorJson = "{\"error\":\"\(error.localizedDescription)\"}"
        errorJson.withCString { errPtr in
            callback(context, errPtr, nil)
        }
    }
}

/// Async streaming implementation
private func generatePersonStreamAsync(
    prompt: String,
    callback: StreamCallback,
    context: UnsafeMutableRawPointer?
) async throws {
    // Create a system language model
    let model = SystemLanguageModel()
    
    // Initialize a session with the model
    let session = LanguageModelSession(model: model, tools: [], instructions: nil)
    
    // Create a prompt from the input string
    let promptObj = Prompt(prompt)
    
    // Generate structured output using streaming
    let options = GenerationOptions()
    let stream = session.streamResponse(
        to: promptObj,
        generating: PersonInfo.self,
        includeSchemaInPrompt: false,
        options: options
    )
    
    // Iterate over the async stream
    for try await snapshot in stream {
        // snapshot.content is PersonInfo.PartiallyGenerated (may have optional fields)
        // We need to convert it to a dictionary and then to JSON
        let contentDict: [String: Any] = [
            "name": snapshot.content.name ?? "",
            "age": snapshot.content.age ?? 0
        ]
        
        let jsonData = try JSONSerialization.data(withJSONObject: contentDict)
        guard let jsonString = String(data: jsonData, encoding: .utf8) else {
            throw NSError(domain: "FoundationModelsBridge", code: -3, userInfo: [
                NSLocalizedDescriptionKey: "failed to encode streaming snapshot to json"
            ])
        }
        
        // rawContent is GeneratedContent, extract jsonString
        let rawText = snapshot.rawContent.jsonString
        
        // Call the callback with partial results
        jsonString.withCString { contentPtr in
            rawText.withCString { rawPtr in
                callback(context, contentPtr, rawPtr)
            }
        }
    }
}
