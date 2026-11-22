import AppKit
import MonitorControlCore
import Observation

/// The app's view of the monitor, read from monitorctl at launch and again in
/// the background each time the panel or Settings opens, and the changes made
/// from either.
@MainActor
@Observable
final class StatusModel {
    private(set) var state: PanelState = .loading
    /// The slider's position; ahead of the monitor while a drag is being written.
    private(set) var brightness: Double = 0
    /// The code of the input being switched to.
    private(set) var switchingTo: String?
    /// Why the last change didn't happen.
    private(set) var actionError: String?
    /// The input row under the pointer.
    var hoveredInput: String?
    /// The code of the input whose name is being edited in place.
    private(set) var renaming: String?
    private(set) var loginItem = LoginItem.state
    /// Why the last Launch at login change didn't happen.
    private(set) var loginItemError: String?
    /// A Retry is reading the monitor.
    private(set) var retrying = false
    /// Why the last name change made in Settings didn't happen, by input code.
    private(set) var nameErrors: [String: String] = [:]

    /// Opens the Settings window; set by the app delegate.
    @ObservationIgnored var openSettings: () -> Void = {}

    @ObservationIgnored weak var panelWindow: NSWindow?
    @ObservationIgnored private var refreshing = false
    @ObservationIgnored private var writer = BrightnessWriter()
    /// The level the monitor last confirmed, to fall back to after a failure.
    @ObservationIgnored private var confirmedBrightness: Int?
    @ObservationIgnored private var tail: Task<Void, Never>?
    @ObservationIgnored private let monitorctl = Monitorctl.bundled()

    /// Reads the monitor straight away, so the panel has it before its first open.
    init() {
        refresh()
    }

    /// Re-reads the monitor, leaving what's on screen in place until the answer arrives.
    func refresh() {
        // Opening the panel can trigger this twice; one refresh at a time.
        guard !refreshing else { return }
        refreshing = true
        actionError = nil
        // May have changed in System Settings since the panel last opened.
        loginItem = LoginItem.state
        loginItemError = nil
        serial { [self] in
            state = PanelState(statusResult: await monitorctl.run(["status", "--json"]))
            // A slider drag during the refresh is newer than what was just read.
            if case .ready(let status) = state, !writer.isWriting {
                confirmedBrightness = status.brightness
                writer = BrightnessWriter(current: status.brightness)
                brightness = Double(status.brightness ?? 0)
            }
            refreshing = false
            retrying = false
        }
    }

    /// Re-reads the monitor after a problem, keeping the problem on screen meanwhile.
    func retry() {
        guard !refreshing else { return }
        retrying = true
        refresh()
    }

    /// Switches input, then closes the menu bar panel and calls `then`.
    func switchInput(to input: InputEntry, then: @escaping () -> Void = {}) {
        guard !input.active, switchingTo == nil, case .ready = state else { return }
        switchingTo = input.code
        actionError = nil
        serial { [self] in
            let result = await monitorctl.run(["input", "set", input.code, "--json"])
            switchingTo = nil
            if let message = failureMessage(of: result) {
                actionError = message
                return
            }
            if case .ready(let status) = state {
                state = .ready(status.switched(to: input.code))
            }
            panelWindow?.close()
            then()
        }
    }

    /// Turns the input's row into a text field for its name.
    func beginRename(_ input: InputEntry) {
        guard switchingTo == nil else { return }
        actionError = nil
        renaming = input.code
    }

    func cancelRename() {
        renaming = nil
    }

    /// Saves the name typed into the row being edited. Only the first of
    /// Return and the field losing focus saves.
    func commitRename(_ input: InputEntry, to text: String) {
        guard renaming == input.code else { return }
        renaming = nil
        actionError = nil
        rename(input, to: text) { [self] in actionError = $0 }
    }

    /// Goes back to the input's built-in name.
    func resetName(_ input: InputEntry) {
        actionError = nil
        rename(input, to: "") { [self] in actionError = $0 }
    }

    /// Saves a name typed in Settings; a failure shows under that input's field.
    func saveName(_ input: InputEntry, to text: String) {
        nameErrors[input.code] = nil
        rename(input, to: text) { [self] in nameErrors[input.code] = $0 }
    }

    /// Shows the new name straight away, then saves it with monitorctl and
    /// puts the old one back (and reports why) if that fails.
    private func rename(_ input: InputEntry, to text: String, failed: @escaping (String) -> Void) {
        let wanted = input.newLabel(from: text)
        guard wanted != input.label else { return }
        relabel(input.code, wanted)
        serial { [self] in
            let result = await monitorctl.run(input.renameArguments(wanted))
            if let message = failureMessage(of: result) {
                relabel(input.code, input.label)
                failed(message)
            } else if let saved = parseRename(result) {
                relabel(saved.code, saved.label)
            }
        }
    }

    private func relabel(_ code: String, _ label: String?) {
        if case .ready(let status) = state {
            state = .ready(status.renamed(code: code, to: label))
        }
    }

    func setBrightness(_ value: Double) {
        brightness = value.rounded()
        actionError = nil
        if let level = writer.want(Int(brightness)) {
            write(level)
        }
    }

    private func write(_ level: Int) {
        serial { [self] in
            let result = await monitorctl.run(["brightness", "\(level)", "--json"])
            if let message = failureMessage(of: result) {
                // Snap back to what the monitor actually has.
                let real = parseBrightness(await monitorctl.run(["brightness", "--json"]))
                confirmedBrightness = real ?? confirmedBrightness
                writer.failed(current: confirmedBrightness)
                brightness = Double(confirmedBrightness ?? 0)
                actionError = message
                return
            }
            confirmedBrightness = level
            if let next = writer.finished() {
                write(next)
            }
        }
    }

    func setLaunchAtLogin(_ on: Bool) {
        do {
            try LoginItem.set(on)
            loginItemError = nil
        } catch {
            loginItemError = "Couldn't change Login Items: \(error.localizedDescription)"
        }
        loginItem = LoginItem.state
    }

    /// Runs monitorctl after every earlier run (for Shortcuts actions), then
    /// re-reads the monitor if it changed something.
    func runQueued(_ args: [String]) async -> RunResult {
        let result = await withCheckedContinuation { continuation in
            serial { [self] in
                continuation.resume(returning: await monitorctl.run(args))
            }
        }
        let changes = args.first == "brightness" && args.count > 2 || args.starts(with: ["input", "set"])
        if changes { refresh() }
        return result
    }

    /// Runs `job` after every earlier one, so only one monitorctl talks to the
    /// monitor at a time.
    private func serial(_ job: @escaping @MainActor () async -> Void) {
        let previous = tail
        tail = Task {
            await previous?.value
            await job()
        }
    }
}
