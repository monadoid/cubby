import Foundation
import Speech
@preconcurrency import AVFoundation
import SwiftRs

// NOTE: runAsyncAndWait is defined in FoundationModelsBridge.swift

// JSON helpers
private func okJSON(_ transcript: String) -> String {
    let obj: [String: Any] = [
        "transcript": transcript
    ]
    let data = try? JSONSerialization.data(withJSONObject: obj, options: [])
    return String(data: data ?? Data("{}".utf8), encoding: .utf8) ?? "{}"
}

private func errJSON(_ message: String) -> String {
    let obj: [String: Any] = [
        "error": message
    ]
    let data = try? JSONSerialization.data(withJSONObject: obj, options: [])
    return String(data: data ?? Data("{}".utf8), encoding: .utf8) ?? "{}"
}

private func okJSONBool() -> String {
    let obj: [String: Any] = [
        "ok": true
    ]
    let data = try? JSONSerialization.data(withJSONObject: obj, options: [])
    return String(data: data ?? Data("{}".utf8), encoding: .utf8) ?? "{}"
}

private func statusString(for status: AssetInventory.Status) -> String {
    switch status {
    case .unsupported:
        return "unsupported"
    case .supported:
        return "supported"
    case .downloading:
        return "downloading"
    case .installed:
        return "installed"
    @unknown default:
        return "unknown"
    }
}

private func statusJSON(_ status: AssetInventory.Status, installedNow: Bool?) -> String {
    var obj: [String: Any] = [
        "status": statusString(for: status)
    ]
    if let installedNow = installedNow {
        obj["installed_now"] = installedNow
    }
    let data = try? JSONSerialization.data(withJSONObject: obj, options: [])
    return String(data: data ?? Data("{}".utf8), encoding: .utf8) ?? "{}"
}

@Sendable
private func ensureSpeechAssets(debugEnabled: Bool) async throws -> (initial: AssetInventory.Status, final: AssetInventory.Status, installedNow: Bool) {
    let (_, transcriber) = try await makeTranscriber()

    let initialStatus = await AssetInventory.status(forModules: [transcriber])
    if debugEnabled {
        print("[speech] assets: initial status \(statusString(for: initialStatus))")
    }

    var installedNow = false
    if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
        if debugEnabled {
            print("[speech] assets: downloading/installing...")
        }
        try await request.downloadAndInstall()
        installedNow = true
    } else if debugEnabled {
        print("[speech] assets: already satisfied")
    }

    let finalStatus = await AssetInventory.status(forModules: [transcriber])
    if debugEnabled {
        print("[speech] assets: final status \(statusString(for: finalStatus))")
    }

    let installedFlag = installedNow && finalStatus == .installed
    return (initial: initialStatus, final: finalStatus, installedNow: installedFlag)
}

@_cdecl("fm_speech_preheat")
public func fm_speech_preheat() -> SRString {
    let debugEnabled = ProcessInfo.processInfo.environment["CUBBY_SPEECH_DEBUG"] != nil

    let result: String = runAsyncAndWait { () async -> String in
        do {
            if debugEnabled { print("[speech] preheat: begin") }

            guard let locale = await SpeechTranscriber.supportedLocale(equivalentTo: Locale.current) else {
                return errJSON("unsupported locale")
            }
            if debugEnabled { print("[speech] preheat: locale \(locale.identifier)") }

            // Enable volatile results for more responsive streaming
            let transcriber = SpeechTranscriber(
                locale: locale,
                transcriptionOptions: [],
                reportingOptions: [.volatileResults],
                attributeOptions: [.audioTimeRange]
            )

            if debugEnabled { print("[speech] preheat: checking assets...") }
            if let req = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
                if debugEnabled { print("[speech] preheat: downloading/installing assets...") }
                try await req.downloadAndInstall()
                if debugEnabled { print("[speech] preheat: assets ready") }
            } else {
                if debugEnabled { print("[speech] preheat: assets already installed") }
            }

            let fmt = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [transcriber])
            let options = SpeechAnalyzer.Options(priority: .userInitiated, modelRetention: .lingering)
            let analyzer = SpeechAnalyzer(modules: [transcriber], options: options)

            try? await analyzer.prepareToAnalyze(in: fmt)
            if debugEnabled { print("[speech] preheat: prepared") }
            if debugEnabled { print("[speech] preheat: done") }

            return okJSONBool()
        } catch {
            if debugEnabled { print("[speech] preheat error: \(error)") }
            return errJSON("speech preheat error: \(error)")
        }
    }
    return SRString(result)
}

@_cdecl("fm_speech_assets_status")
public func fm_speech_assets_status() -> SRString {
    let debugEnabled = ProcessInfo.processInfo.environment["CUBBY_SPEECH_DEBUG"] != nil

    let result: String = runAsyncAndWait { () async -> String in
        do {
            if debugEnabled { print("[speech] assets status: begin") }
            let (_, transcriber) = try await makeTranscriber()
            let status = await AssetInventory.status(forModules: [transcriber])
            if debugEnabled { print("[speech] assets status: \(statusString(for: status))") }
            return statusJSON(status, installedNow: nil)
        } catch {
            if debugEnabled { print("[speech] assets status error: \(error)") }
            return errJSON("speech assets status error: \(error)")
        }
    }
    return SRString(result)
}

@_cdecl("fm_speech_ensure_assets")
public func fm_speech_ensure_assets() -> SRString {
    let debugEnabled = ProcessInfo.processInfo.environment["CUBBY_SPEECH_DEBUG"] != nil

    let result: String = runAsyncAndWait { () async -> String in
        do {
            if debugEnabled { print("[speech] ensure assets: begin") }
            let result = try await ensureSpeechAssets(debugEnabled: debugEnabled)
            if debugEnabled {
                print("[speech] ensure assets: installed now = \(result.installedNow)")
            }
            return statusJSON(result.final, installedNow: result.installedNow)
        } catch {
            if debugEnabled { print("[speech] ensure assets error: \(error)") }
            return errJSON("speech ensure assets error: \(error)")
        }
    }
    return SRString(result)
}

@_cdecl("fm_speech_install_assets")
public func fm_speech_install_assets() -> SRString {
    let debugEnabled = ProcessInfo.processInfo.environment["CUBBY_SPEECH_DEBUG"] != nil

    let result: String = runAsyncAndWait { () async -> String in
        do {
            if debugEnabled { print("[speech] assets: begin") }
            let result = try await ensureSpeechAssets(debugEnabled: debugEnabled)
            switch result.final {
            case .installed:
                if debugEnabled { print("[speech] assets: ready") }
                return okJSONBool()
            case .downloading:
                if debugEnabled { print("[speech] assets: download still in progress") }
                return okJSONBool()
            case .supported, .unsupported:
                if debugEnabled { print("[speech] assets: final status \(statusString(for: result.final))") }
                return errJSON("speech assets error: final status \(statusString(for: result.final))")
            @unknown default:
                if debugEnabled { print("[speech] assets: unexpected status \(statusString(for: result.final))") }
                return errJSON("speech assets error: unexpected status \(statusString(for: result.final))")
            }
        } catch {
            if debugEnabled { print("[speech] assets error: \(error)") }
            return errJSON("speech assets error: \(error)")
        }
    }
    return SRString(result)
}

@_cdecl("fm_speech_transcribe_file")
public func fm_speech_transcribe_file(path: SRString) -> SRString {
    let str = path.toString()
    guard !str.isEmpty else {
        return SRString(errJSON("empty path"))
    }
    
    let debugEnabled = ProcessInfo.processInfo.environment["CUBBY_SPEECH_DEBUG"] != nil
    
    let result: String = runAsyncAndWait { () async -> String in
        do {
            let url = URL(fileURLWithPath: str)
            if debugEnabled { print("[speech] transcribing: \(url.path)") }
            
            let (locale, transcriber) = try await makeTranscriber()
            if debugEnabled { print("[speech] locale: \(locale.identifier)") }
            
            // ensure assets
            if debugEnabled { print("[speech] checking assets...") }
            if let req = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
                if debugEnabled { print("[speech] downloading/installing assets...") }
                try await req.downloadAndInstall()
                if debugEnabled { print("[speech] assets ready") }
            } else {
                if debugEnabled { print("[speech] assets already installed") }
            }
            
            // build analyzer
            let audioFormat = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [transcriber])
            let analyzer = SpeechAnalyzer(modules: [transcriber])
            // preheat resources to reduce first-token latency
            try? await analyzer.prepareToAnalyze(in: audioFormat)
            
            if debugEnabled { print("[speech] starting analyzer...") }
            
            // start analysis from file
            let audioFile = try AVAudioFile(forReading: url)
            
            // CRITICAL: drain results concurrently while analyzer runs
            let drainTask = Task { () -> (String, CMTime?) in
                var collected = ""
                var lastTime: CMTime? = nil
                do {
                    for try await result in transcriber.results {
                        let best = result.text
                        let plain = String(best.characters)
                        collected += (collected.isEmpty ? "" : " ") + plain
                        lastTime = result.resultsFinalizationTime
                        if debugEnabled {
                            print("[speech] result chunk: \(plain.count) chars, time: \(lastTime?.seconds ?? 0)")
                        }
                    }
                } catch {
                    if debugEnabled { print("[speech] results stream error: \(error)") }
                }
                return (collected, lastTime)
            }
            
            // run analyzer (blocks until file processed)
            let _ = try await analyzer.analyzeSequence(from: audioFile)
            if debugEnabled { print("[speech] analyzer sequence complete, finalizing...") }
            
            // finalize to flush remaining results
            try await analyzer.finalizeAndFinishThroughEndOfInput()
            if debugEnabled { print("[speech] finalized, waiting for drain...") }
            
            // await drain completion
            let (collected, _) = await drainTask.value
            if debugEnabled { print("[speech] transcription complete: \(collected.count) chars") }
            
            return okJSON(collected)
        } catch {
            if debugEnabled { print("[speech] error: \(error)") }
            return errJSON("speech error: \(error)")
        }
    }
    return SRString(result)
}

private func makeTranscriber() async throws -> (Locale, SpeechTranscriber) {
    guard let locale = await SpeechTranscriber.supportedLocale(equivalentTo: Locale.current) else {
        throw NSError(domain: "SpeechBridge", code: 1, userInfo: [NSLocalizedDescriptionKey: "unsupported locale"])
    }
    // Enable volatile results for more responsive streaming
    let transcriber = SpeechTranscriber(
        locale: locale, 
        transcriptionOptions: [],
        reportingOptions: [.volatileResults],
        attributeOptions: [.audioTimeRange]
    )
    return (locale, transcriber)
}

@_cdecl("fm_speech_supported_locale")
public func fm_speech_supported_locale() -> SRString {
    let result: String = runAsyncAndWait { () async -> String in
        guard let l = await SpeechTranscriber.supportedLocale(equivalentTo: Locale.current) else {
            return errJSON("unsupported locale")
        }
        let obj: [String: Any] = ["locale": l.identifier]
        let data = try? JSONSerialization.data(withJSONObject: obj, options: [])
        return String(data: data ?? Data("{}".utf8), encoding: .utf8) ?? "{}"
    }
    return SRString(result)
}

// MARK: - Streaming support

private enum StreamingError: Error, CustomStringConvertible {
    case unsupportedLocale
    case invalidSession
    case invalidSampleRate
    case streamClosed
    case converterFailure(String)

    var description: String {
        switch self {
        case .unsupportedLocale:
            return "unsupported locale"
        case .invalidSession:
            return "invalid session id"
        case .invalidSampleRate:
            return "invalid audio sample rate"
        case .streamClosed:
            return "stream already finished"
        case .converterFailure(let message):
            return "audio conversion failed: \(message)"
        }
    }
}

fileprivate struct TranscriberResultEnvelope: Codable, Sendable {
    enum Event: String, Codable {
        case partial
        case final
        case done
        case timeout
        case error
    }

    var event: Event
    var text: String?
    var isFinal: Bool?
    var timestamp: Double?
    var error: String?

    static func timeout() -> TranscriberResultEnvelope {
        TranscriberResultEnvelope(event: .timeout, text: nil, isFinal: nil, timestamp: nil, error: nil)
    }

    static func done() -> TranscriberResultEnvelope {
        TranscriberResultEnvelope(event: .done, text: nil, isFinal: nil, timestamp: nil, error: nil)
    }

    static func error(_ message: String) -> TranscriberResultEnvelope {
        TranscriberResultEnvelope(event: .error, text: nil, isFinal: nil, timestamp: nil, error: message)
    }
}

private actor StreamingSession {
    let id: UUID
    private let transcriber: SpeechTranscriber
    private let analyzer: SpeechAnalyzer
    private let targetFormat: AVAudioFormat

    private let inputStream: AsyncStream<AnalyzerInput>
    private var inputContinuation: AsyncStream<AnalyzerInput>.Continuation

    private var analyzerTask: Task<Void, Never>?
    private var resultsTask: Task<Void, Never>?

    private var resultsQueue: [TranscriberResultEnvelope] = []
    private var waiterOrder: [UUID] = []
    private var waiterContinuations: [UUID: CheckedContinuation<TranscriberResultEnvelope, Never>] = [:]

    private var inputFinished = false
    private var hasSentDone = false

    init(id: UUID, transcriber: SpeechTranscriber, analyzer: SpeechAnalyzer, targetFormat: AVAudioFormat) {
        self.id = id
        self.transcriber = transcriber
        self.analyzer = analyzer
        self.targetFormat = targetFormat

        var capturedContinuation: AsyncStream<AnalyzerInput>.Continuation!
        self.inputStream = AsyncStream<AnalyzerInput> { continuation in
            capturedContinuation = continuation
        }
        capturedContinuation.onTermination = { _ in }
        self.inputContinuation = capturedContinuation
    }

    deinit {
        analyzerTask?.cancel()
        resultsTask?.cancel()
    }

    func activate() {
        resultsTask = Task { [weak self] in
            guard let session = self else { return }
            await session.drainResults()
        }

        analyzerTask = Task { [weak self] in
            guard let session = self else { return }
            await session.runAnalyzer()
        }
    }

    func push(samples: [Float], sampleRate: Int) async throws {
        guard !inputFinished else { throw StreamingError.streamClosed }
        guard sampleRate > 0 else { throw StreamingError.invalidSampleRate }
        guard !samples.isEmpty else { return }

        let buffer = try makePCMBuffer(samples: samples, sampleRate: sampleRate)
        let converted = try convert(buffer: buffer)
        inputContinuation.yield(AnalyzerInput(buffer: converted))
    }

    func finish() async throws {
        guard !inputFinished else { return }
        inputFinished = true
        inputContinuation.finish()
        do {
            try await analyzer.finalizeAndFinishThroughEndOfInput()
        } catch {
            enqueue(.error("analyzer finalize failed: \(error)"))
        }
    }

    func cancel() async {
        guard !inputFinished else {
            print("[Swift] Session already cancelled, ensuring done is enqueued")
            await ensureDoneEnqueued()
            return
        }
        print("[Swift] Cancelling session...")
        inputFinished = true
        inputContinuation.finish()
        print("[Swift] Calling analyzer.cancelAndFinishNow()")
        await analyzer.cancelAndFinishNow()
        print("[Swift] Ensuring done is enqueued")
        await ensureDoneEnqueued()
        print("[Swift] Session cancellation complete")
    }

    func nextResult(timeout: Duration) async -> TranscriberResultEnvelope {
        if let queued = dequeueResult() {
            return queued
        }
        if hasSentDone {
            return .done()
        }
        let waiterId = UUID()
        return await withCheckedContinuation { continuation in
            waiterOrder.append(waiterId)
            waiterContinuations[waiterId] = continuation
            scheduleTimeout(timeout, waiterId: waiterId)
        }
    }

    private func runAnalyzer() async {
        do {
            _ = try await analyzer.analyzeSequence(inputStream)
            await ensureDoneEnqueued()
        } catch {
            enqueue(.error("analyzer error: \(error)"))
        }
    }

    private func drainResults() async {
        do {
            for try await result in transcriber.results {
                let text = String(result.text.characters)
                let timestamp = result.resultsFinalizationTime.seconds
                let event: TranscriberResultEnvelope.Event = result.isFinal ? .final : .partial
                let envelope = TranscriberResultEnvelope(
                    event: event,
                    text: text,
                    isFinal: result.isFinal,
                    timestamp: timestamp.isFinite ? timestamp : nil,
                    error: nil
                )
                enqueue(envelope)
            }
            await ensureDoneEnqueued()
        } catch {
            enqueue(.error("transcriber error: \(error)"))
        }
    }

    private func ensureDoneEnqueued() async {
        if hasSentDone { return }
        hasSentDone = true
        enqueue(.done())
    }

    private func enqueue(_ envelope: TranscriberResultEnvelope) {
        if !waiterOrder.isEmpty {
            let id = waiterOrder.removeFirst()
            if let continuation = waiterContinuations.removeValue(forKey: id) {
                continuation.resume(returning: envelope)
                return
            }
        }
        resultsQueue.append(envelope)
    }

    private func dequeueResult() -> TranscriberResultEnvelope? {
        if !resultsQueue.isEmpty {
            return resultsQueue.removeFirst()
        }
        return nil
    }

    private func scheduleTimeout(_ timeout: Duration, waiterId: UUID) {
        let nanos = max(toNanoseconds(timeout), 0)
        if nanos == 0 {
            expireWaiter(waiterId: waiterId)
            return
        }
        Task {
            try? await Task.sleep(nanoseconds: UInt64(nanos))
            await self.expireWaiter(waiterId: waiterId)
        }
    }

    private func expireWaiter(waiterId: UUID) {
        guard let continuation = waiterContinuations.removeValue(forKey: waiterId) else { return }
        if let idx = waiterOrder.firstIndex(of: waiterId) {
            waiterOrder.remove(at: idx)
        }
        continuation.resume(returning: .timeout())
    }

    private func toNanoseconds(_ duration: Duration) -> Int64 {
        let components = duration.components
        let seconds = components.seconds
        let attoseconds = components.attoseconds
        let (secNanos, overflow) = seconds.multipliedReportingOverflow(by: 1_000_000_000)
        if overflow {
            return seconds >= 0 ? Int64.max : Int64.min
        }
        let attosecondNanos = attoseconds / 1_000_000_000
        let (total, overflowTotal) = secNanos.addingReportingOverflow(attosecondNanos)
        if overflowTotal {
            return total >= 0 ? Int64.max : Int64.min
        }
        return total
    }

    private func makePCMBuffer(samples: [Float], sampleRate: Int) throws -> AVAudioPCMBuffer {
        guard let format = AVAudioFormat(standardFormatWithSampleRate: Double(sampleRate), channels: 1) else {
            throw StreamingError.converterFailure("could not build source format")
        }
        guard let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(samples.count)) else {
            throw StreamingError.converterFailure("could not allocate pcm buffer")
        }
        buffer.frameLength = AVAudioFrameCount(samples.count)
        guard let channel = buffer.floatChannelData else {
            throw StreamingError.converterFailure("missing channel data")
        }
        samples.withUnsafeBufferPointer { pointer in
            guard let base = pointer.baseAddress else { return }
            channel[0].update(from: base, count: samples.count)
        }
        return buffer
    }

    private func convert(buffer: AVAudioPCMBuffer) throws -> AVAudioPCMBuffer {
        if buffer.format == targetFormat {
            return buffer
        }
        guard let converter = AVAudioConverter(from: buffer.format, to: targetFormat) else {
            throw StreamingError.converterFailure("unable to create converter")
        }
        let ratio = targetFormat.sampleRate / buffer.format.sampleRate
        let estimatedFrames = max(1, Int((Double(buffer.frameLength) * ratio).rounded(.up)))
        guard let output = AVAudioPCMBuffer(pcmFormat: targetFormat, frameCapacity: AVAudioFrameCount(estimatedFrames)) else {
            throw StreamingError.converterFailure("unable to allocate converted buffer")
        }
        var conversionError: NSError?
        let status = converter.convert(to: output, error: &conversionError) { _, status in
            status.pointee = .haveData
            return buffer
        }
        switch status {
        case .haveData, .inputRanDry, .endOfStream:
            return output
        case .error:
            let message = conversionError?.localizedDescription ?? "unknown"
            throw StreamingError.converterFailure(message)
        @unknown default:
            throw StreamingError.converterFailure("unexpected converter status")
        }
    }
}

private actor StreamingSessionManager {
    static let shared = StreamingSessionManager()

    private var sessions: [UUID: StreamingSession] = [:]

    func createSession() async throws -> UUID {
        guard let locale = await SpeechTranscriber.supportedLocale(equivalentTo: Locale.current) else {
            throw StreamingError.unsupportedLocale
        }
        // Enable volatile results for more responsive streaming
        let transcriber = SpeechTranscriber(
            locale: locale, 
            transcriptionOptions: [],
            reportingOptions: [.volatileResults],
            attributeOptions: [.audioTimeRange]
        )
        if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
            try await request.downloadAndInstall()
        }
        guard let format = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [transcriber]) else {
            throw StreamingError.converterFailure("unable to determine audio format")
        }
        let options = SpeechAnalyzer.Options(priority: .userInitiated, modelRetention: .lingering)
        let analyzer = SpeechAnalyzer(modules: [transcriber], options: options)
        try? await analyzer.prepareToAnalyze(in: format)

        let identifier = UUID()
        let session = StreamingSession(
            id: identifier,
            transcriber: transcriber,
            analyzer: analyzer,
            targetFormat: format
        )
        await session.activate()
        sessions[identifier] = session
        return identifier
    }

    func push(id: UUID, samples: [Float], sampleRate: Int) async throws {
        guard let session = sessions[id] else {
            throw StreamingError.invalidSession
        }
        try await session.push(samples: samples, sampleRate: sampleRate)
    }

    func finish(id: UUID) async throws {
        guard let session = sessions[id] else {
            throw StreamingError.invalidSession
        }
        try await session.finish()
    }

    func cancel(id: UUID) async {
        print("[Swift] StreamingSessionManager: Cancelling session \(id)")
        guard let session = sessions[id] else {
            print("[Swift] StreamingSessionManager: Session \(id) not found")
            return
        }
        print("[Swift] StreamingSessionManager: Found session, calling cancel()")
        await session.cancel()
        sessions[id] = nil
        print("[Swift] StreamingSessionManager: Session \(id) removed from sessions")
    }

    func nextResult(id: UUID, timeoutMs: Int) async -> TranscriberResultEnvelope {
        guard let session = sessions[id] else {
            return .error(StreamingError.invalidSession.description)
        }
        let clamped = max(timeoutMs, 0)
        let envelope = await session.nextResult(timeout: .milliseconds(clamped))
        switch envelope.event {
        case .done, .error:
            sessions[id] = nil
        default:
            break
        }
        return envelope
    }
}

private func streamingJSON(_ payload: [String: Any]) -> String {
    guard JSONSerialization.isValidJSONObject(payload) else { return "{}" }
    let data = try? JSONSerialization.data(withJSONObject: payload, options: [])
    return String(data: data ?? Data("{}".utf8), encoding: .utf8) ?? "{}"
}

@_cdecl("fm_speech_stream_create")
public func fm_speech_stream_create() -> SRString {
    let result: String = runAsyncAndWait {
        do {
            let id = try await StreamingSessionManager.shared.createSession()
            return streamingJSON(["session_id": id.uuidString])
        } catch let error as StreamingError {
            return errJSON(error.description)
        } catch {
            return errJSON("speech stream create error: \(error)")
        }
    }
    return SRString(result)
}

@_cdecl("fm_speech_stream_push_f32")
public func fm_speech_stream_push_f32(
    session_id: SRString,
    samples_ptr: UnsafePointer<UInt8>,
    samples_len: Int,
    sample_rate: Int32
) -> SRString {
    let session = session_id.toString()
    guard let id = UUID(uuidString: session) else { return SRString(errJSON(StreamingError.invalidSession.description)) }
    guard samples_len >= 0 else { return SRString(errJSON("speech stream push error: invalid buffer length")) }
    let byteCount = samples_len
    guard byteCount % MemoryLayout<Float>.stride == 0 else {
        return SRString(errJSON("speech stream push error: misaligned buffer"))
    }
    let floatCount = byteCount / MemoryLayout<Float>.stride
    let floats: [Float]
    if floatCount > 0 {
        floats = samples_ptr.withMemoryRebound(to: Float.self, capacity: floatCount) {
            Array(UnsafeBufferPointer(start: $0, count: floatCount))
        }
    } else {
        floats = []
    }
    let result: String = runAsyncAndWait {
        do {
            try await StreamingSessionManager.shared.push(id: id, samples: floats, sampleRate: Int(sample_rate))
            return okJSONBool()
        } catch let error as StreamingError {
            return errJSON(error.description)
        } catch {
            return errJSON("speech stream push error: \(error)")
        }
    }
    return SRString(result)
}

@_cdecl("fm_speech_stream_finish")
public func fm_speech_stream_finish(session_id: SRString) -> SRString {
    let session = session_id.toString()
    guard let id = UUID(uuidString: session) else { return SRString(errJSON(StreamingError.invalidSession.description)) }
    let result: String = runAsyncAndWait {
        do {
            try await StreamingSessionManager.shared.finish(id: id)
            return okJSONBool()
        } catch let error as StreamingError {
            return errJSON(error.description)
        } catch {
            return errJSON("speech stream finish error: \(error)")
        }
    }
    return SRString(result)
}

@_cdecl("fm_speech_stream_cancel")
public func fm_speech_stream_cancel(session_id: SRString) -> SRString {
    let session = session_id.toString()
    guard let id = UUID(uuidString: session) else { return SRString(errJSON(StreamingError.invalidSession.description)) }
    let result: String = runAsyncAndWait {
        await StreamingSessionManager.shared.cancel(id: id)
        return okJSONBool()
    }
    return SRString(result)
}

@_cdecl("fm_speech_stream_next_result")
public func fm_speech_stream_next_result(session_id: SRString, timeout_ms: Int32) -> SRString {
    let session = session_id.toString()
    guard let id = UUID(uuidString: session) else { return SRString(errJSON(StreamingError.invalidSession.description)) }
    let result: String = runAsyncAndWait {
        let envelope = await StreamingSessionManager.shared.nextResult(id: id, timeoutMs: Int(timeout_ms))
        let encoder = JSONEncoder()
        if let data = try? encoder.encode(envelope),
           let string = String(data: data, encoding: .utf8) {
            return string
        }
        return errJSON("failed to encode streaming result")
    }
    return SRString(result)
}
