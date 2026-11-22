import AppKit
import MonitorControlCore
import SwiftUI

/// The small panel a Control Center control opens: the brightness slider or
/// the input list. A borderless floating panel at the top right of the screen
/// with the pointer; clicking away or pressing Esc closes it.
@MainActor
final class ControlPanel: NSObject, NSWindowDelegate {
    enum Kind {
        case brightness
        case inputs
    }

    private let model: StatusModel
    private var kind = Kind.brightness
    private var screen: NSScreen?

    private lazy var panel: KeyPanel = {
        let panel = KeyPanel(
            contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered, defer: true)
        panel.isFloatingPanel = true
        panel.level = .popUpMenu
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = true
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.delegate = self
        return panel
    }()

    init(model: StatusModel) {
        self.model = model
    }

    func show(_ kind: Kind) {
        self.kind = kind
        let mouse = NSEvent.mouseLocation
        screen = NSScreen.screens.first { $0.frame.contains(mouse) } ?? NSScreen.main
        let host = NSHostingController(
            rootView: ControlPanelView(model: model, kind: kind, close: { [weak self] in self?.close() }))
        // Size the panel to its content, and follow later changes in height.
        host.sizingOptions = [.preferredContentSize]
        panel.contentViewController = host
        place()
        model.refresh()
        NSApp.activate()
        panel.makeKeyAndOrderFront(nil)
    }

    func close() {
        panel.orderOut(nil)
    }

    /// Keeps the panel pinned to the top-right corner as its height changes.
    private func place() {
        guard let visible = (screen ?? panel.screen)?.visibleFrame else { return }
        panel.setFrameOrigin(panelOrigin(size: panel.frame.size, visibleFrame: visible))
    }

    func windowDidResize(_ notification: Notification) {
        place()
    }

    func windowDidResignKey(_ notification: Notification) {
        close()
    }
}

/// A borderless panel that can take key focus (for Esc and the slider).
final class KeyPanel: NSPanel {
    override var canBecomeKey: Bool { true }

    override func cancelOperation(_ sender: Any?) {
        orderOut(nil)
    }
}

struct ControlPanelView: View {
    let model: StatusModel
    let kind: ControlPanel.Kind
    let close: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title).font(.headline).foregroundStyle(monitorName == nil ? .secondary : .primary)
            Divider()
            content
            if let error = model.actionError {
                Text(error)
                    .font(.callout)
                    .foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(14)
        .frame(width: 280, alignment: .leading)
        .fixedSize(horizontal: false, vertical: true)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12))
    }

    private var monitorName: String? {
        if case .ready(let status) = model.state { status.name } else { nil }
    }

    private var title: String {
        monitorName ?? "Monitor Control"
    }

    @ViewBuilder
    private var content: some View {
        switch (kind, model.state) {
        case (.brightness, .loading):
            BrightnessSection(model: model, available: true).redacted(reason: .placeholder).disabled(true)
        case (.brightness, .ready(let status)):
            BrightnessSection(model: model, available: status.brightness != nil)
        case (.brightness, .problem(.noMonitor)):
            NoMonitorBrightness()
        case (.inputs, .loading):
            InputSection(model: model, inputs: InputSection.placeholder, allowsRename: false)
                .redacted(reason: .placeholder).disabled(true)
        case (.inputs, .ready(let status)):
            InputSection(model: model, inputs: status.inputs, allowsRename: false, onSwitched: close)
            Button("Rename inputs in Settings…") {
                close()
                model.openSettings()
            }
            .buttonStyle(.link)
            .font(.callout)
        case (.inputs, .problem(.noMonitor)):
            VStack(alignment: .leading, spacing: 6) {
                SectionTitle("Input")
                Text("No monitor detected").foregroundStyle(.secondary)
            }
        case (_, .problem(let problem)):
            ProblemView(problem: problem, retrying: model.retrying, retry: model.retry)
        }
    }
}

/// The brightness section with no monitor: the slider stays in place, disabled.
private struct NoMonitorBrightness: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            SectionTitle("Brightness")
            HStack(spacing: 8) {
                Image(systemName: "sun.min").foregroundStyle(.secondary)
                Slider(value: .constant(0), in: 0...100).disabled(true)
                Image(systemName: "sun.max").foregroundStyle(.secondary)
                Text("–").monospacedDigit().frame(width: 28, alignment: .trailing)
            }
            Text("No monitor detected").foregroundStyle(.secondary)
        }
    }
}
