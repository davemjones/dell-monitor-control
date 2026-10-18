import AppKit
import MonitorControlCore
import Observation

/// The panel's view of the monitor, read from monitorctl at launch and again in
/// the background each time the panel opens, and the changes made from it.
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
    private(set) var loginItem = LoginItem.state
    /// Why the last Launch at login change didn't happen.
    private(set) var loginItemError: String?

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
        }
    }

    /// Re-reads the monitor after a problem, showing the loading state meanwhile.
    func retry() {
        guard !refreshing else { return }
        state = .loading
        refresh()
    }

    func switchInput(to input: InputEntry) {
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
