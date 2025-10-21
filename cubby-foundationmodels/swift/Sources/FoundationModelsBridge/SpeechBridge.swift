import Foundation
import Speech
import AVFoundation

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

public func fm_speech_preheat() -> String {
    let debugEnabled = ProcessInfo.processInfo.environment["CUBBY_SPEECH_DEBUG"] != nil

    return runAsyncAndWait { () async -> String in
        do {
            if debugEnabled { print("[speech] preheat: begin") }

            guard let locale = await SpeechTranscriber.supportedLocale(equivalentTo: Locale.current) else {
                return errJSON("unsupported locale")
            }
            if debugEnabled { print("[speech] preheat: locale \(locale.identifier)") }

            let transcriber = SpeechTranscriber(locale: locale, preset: .transcription)

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
}

public func fm_speech_install_assets() -> String {
    let debugEnabled = ProcessInfo.processInfo.environment["CUBBY_SPEECH_DEBUG"] != nil

    return runAsyncAndWait { () async -> String in
        do {
            if debugEnabled { print("[speech] assets: begin") }
            guard let locale = await SpeechTranscriber.supportedLocale(equivalentTo: Locale.current) else {
                return errJSON("unsupported locale")
            }
            let transcriber = SpeechTranscriber(locale: locale, preset: .transcription)
            if let req = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
                if debugEnabled { print("[speech] assets: downloading/installing...") }
                try await req.downloadAndInstall()
                if debugEnabled { print("[speech] assets: ready") }
            } else {
                if debugEnabled { print("[speech] assets: already installed") }
            }
            return okJSONBool()
        } catch {
            if debugEnabled { print("[speech] assets error: \(error)") }
            return errJSON("speech assets error: \(error)")
        }
    }
}

public func fm_speech_transcribe_file(path: RustStr) -> String {
    let str = path.toString()
    guard !str.isEmpty else {
        return errJSON("empty path")
    }
    
    let debugEnabled = ProcessInfo.processInfo.environment["CUBBY_SPEECH_DEBUG"] != nil
    
    return runAsyncAndWait { () async -> String in
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
}

private func makeTranscriber() async throws -> (Locale, SpeechTranscriber) {
    guard let locale = await SpeechTranscriber.supportedLocale(equivalentTo: Locale.current) else {
        throw NSError(domain: "SpeechBridge", code: 1, userInfo: [NSLocalizedDescriptionKey: "unsupported locale"])
    }
    let transcriber = SpeechTranscriber(locale: locale, preset: .transcription)
    return (locale, transcriber)
}

public func fm_speech_supported_locale() -> String {
    return runAsyncAndWait { () async -> String in
        guard let l = await SpeechTranscriber.supportedLocale(equivalentTo: Locale.current) else {
            return errJSON("unsupported locale")
        }
        let obj: [String: Any] = ["locale": l.identifier]
        let data = try? JSONSerialization.data(withJSONObject: obj, options: [])
        return String(data: data ?? Data("{}".utf8), encoding: .utf8) ?? "{}"
    }
}


