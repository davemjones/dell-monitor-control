import Foundation

/// What happened when the app ran monitorctl.
public enum RunResult: Sendable, Equatable {
    case finished(exitCode: Int32, stdout: Data, stderr: Data)
    case launchFailed
    case timedOut
}

/// The monitorctl command-line tool bundled inside the app. All monitor reading
/// and control goes through it.
public struct Monitorctl: Sendable {
    public let executable: URL

    public init(executable: URL) {
        self.executable = executable
    }

    /// The copy at `Monitor Control.app/Contents/Helpers/monitorctl`.
    public static func bundled(in bundle: Bundle = .main) -> Monitorctl {
        Monitorctl(executable: bundle.bundleURL.appending(path: "Contents/Helpers/monitorctl"))
    }

    /// Runs monitorctl with `args`, giving up (and stopping it) after `timeout` seconds.
    public func run(_ args: [String], timeout: TimeInterval = 10) async -> RunResult {
        let job = Job(executable: executable, args: args, timeout: timeout)
        return await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                continuation.resume(returning: job.run())
            }
        }
    }

    /// One blocking run of the process, done off the main thread.
    private final class Job: @unchecked Sendable {
        private let process = Process()
        private let timeout: TimeInterval
        private let lock = NSLock()
        private var didTimeOut = false
        /// Written by the stderr reader before it signals `drained`.
        private var stderr = Data()

        init(executable: URL, args: [String], timeout: TimeInterval) {
            process.executableURL = executable
            process.arguments = args
            self.timeout = timeout
        }

        func run() -> RunResult {
            let out = Pipe()
            let err = Pipe()
            process.standardOutput = out
            process.standardError = err
            process.standardInput = FileHandle.nullDevice
            do {
                try process.run()
            } catch {
                return .launchFailed
            }

            let watchdog = DispatchWorkItem { [self] in
                lock.withLock { didTimeOut = true }
                process.terminate()
            }
            DispatchQueue.global().asyncAfter(deadline: .now() + timeout, execute: watchdog)

            // Drain stderr alongside stdout so neither pipe can fill up and stall monitorctl.
            let drained = DispatchSemaphore(value: 0)
            DispatchQueue.global().async { [self] in
                stderr = err.fileHandleForReading.readDataToEndOfFile()
                drained.signal()
            }
            let stdout = out.fileHandleForReading.readDataToEndOfFile()
            drained.wait()
            process.waitUntilExit()
            watchdog.cancel()

            if lock.withLock({ didTimeOut }) {
                return .timedOut
            }
            return .finished(exitCode: process.terminationStatus, stdout: stdout, stderr: stderr)
        }
    }
}
