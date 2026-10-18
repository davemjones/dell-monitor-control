import AppKit
import MonitorControlCore
import SwiftUI

struct PanelView: View {
    let model: StatusModel

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            header
            Divider()
            switch model.state {
            case .loading:
                InputSection(model: model, inputs: InputSection.placeholder)
                    .redacted(reason: .placeholder)
                    .disabled(true)
                BrightnessSection(model: model, available: true)
                    .redacted(reason: .placeholder)
                    .disabled(true)
            case .ready(let status):
                InputSection(model: model, inputs: status.inputs)
                BrightnessSection(model: model, available: status.brightness != nil)
                if let error = model.actionError {
                    Text(error)
                        .font(.callout)
                        .foregroundStyle(.red)
                        .fixedSize(horizontal: false, vertical: true)
                }
            case .problem(let problem):
                ProblemView(problem: problem, retrying: model.retrying, retry: model.retry)
            }
            Divider()
            Footer(model: model)
        }
        .padding(14)
        .frame(width: 280)
        .fixedSize(horizontal: false, vertical: true)
        // The menu bar extra only sizes its window as it opens, so follow later
        // changes in the content's height ourselves.
        .onGeometryChange(for: CGFloat.self, of: \.size.height) { height in
            fit(model.panelWindow, toContentHeight: height)
        }
        // Keep the content under the menu bar if the window is ever taller.
        .frame(maxHeight: .infinity, alignment: .top)
        .onAppear(perform: model.refresh)
        .background(
            PanelOpenObserver { window in
                model.panelWindow = window
                model.refresh()
            })
    }

    private var header: some View {
        HStack {
            if case .ready(let status) = model.state {
                Text(status.name).font(.headline)
            } else {
                Text("Monitor Control").font(.headline).foregroundStyle(.secondary)
            }
            Spacer()
            if model.state == .loading || model.retrying {
                ProgressView().controlSize(.small)
            }
        }
    }
}

/// Resizes the open panel to fit `height` of content, keeping its top edge
/// against the menu bar.
@MainActor
private func fit(_ window: NSWindow?, toContentHeight height: CGFloat) {
    guard let window, window.isVisible else { return }
    let content = window.contentRect(forFrameRect: window.frame)
    let target = (height + window.frame.height - content.height).rounded()
    guard abs(window.frame.height - target) >= 1 else { return }
    var frame = window.frame
    frame.origin.y = frame.maxY - target
    frame.size.height = target
    window.setFrame(frame, display: true)
}

private struct Footer: View {
    let model: StatusModel

    private static let version =
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "dev"

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Toggle(
                "Launch at login",
                isOn: Binding(get: { model.loginItem == .on }, set: model.setLaunchAtLogin)
            )
            .toggleStyle(.checkbox)
            if model.loginItem == .needsApproval {
                Button("Allow in Login Items…", action: LoginItem.openSettings)
                    .buttonStyle(.link)
                    .font(.callout)
            }
            if let error = model.loginItemError {
                Text(error)
                    .font(.callout)
                    .foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }
            HStack {
                Button("Quit Monitor Control") { NSApplication.shared.terminate(nil) }
                    .buttonStyle(.plain)
                Spacer()
                Text("Version \(Self.version)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }
}

private struct InputSection: View {
    static let placeholder = ["thunderbolt", "dp", "hdmi"].map {
        InputEntry(name: $0, code: "", active: false)
    }

    let model: StatusModel
    let inputs: [InputEntry]?

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            SectionTitle("Input").padding(.bottom, 4)
            if let inputs {
                ForEach(inputs, id: \.name) { input in
                    row(input)
                }
            } else {
                NotAvailable()
            }
        }
    }
}

extension InputSection {
    private func row(_ input: InputEntry) -> some View {
        let switching = model.switchingTo == input.code
        return Button {
            model.switchInput(to: input)
        } label: {
            HStack(spacing: 6) {
                Image(systemName: "checkmark")
                    .font(.caption.weight(.semibold))
                    .opacity(input.active ? 1 : 0)
                Text(input.label)
                Spacer()
                if switching {
                    ProgressView().controlSize(.mini)
                }
            }
            .padding(.vertical, 4)
            .padding(.horizontal, 6)
            .contentShape(Rectangle())
            .background(
                RoundedRectangle(cornerRadius: 5)
                    .fill(.primary.opacity(highlighted(input) ? 0.1 : 0))
            )
        }
        .buttonStyle(.plain)
        .disabled(model.switchingTo != nil && !switching)
        .onHover { inside in
            if inside {
                model.hoveredInput = input.code
            } else if model.hoveredInput == input.code {
                model.hoveredInput = nil
            }
        }
    }

    private func highlighted(_ input: InputEntry) -> Bool {
        !input.active && model.switchingTo == nil && model.hoveredInput == input.code
    }
}

private struct BrightnessSection: View {
    let model: StatusModel
    let available: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            SectionTitle("Brightness")
            if available {
                HStack(spacing: 8) {
                    Image(systemName: "sun.min").foregroundStyle(.secondary)
                    Slider(
                        value: Binding(get: { model.brightness }, set: model.setBrightness),
                        in: 0...100
                    )
                    Image(systemName: "sun.max").foregroundStyle(.secondary)
                    Text("\(Int(model.brightness))")
                        .monospacedDigit()
                        .frame(width: 28, alignment: .trailing)
                }
            } else {
                NotAvailable()
            }
        }
    }
}

private struct ProblemView: View {
    let problem: Problem
    let retrying: Bool
    let retry: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label(problem.title, systemImage: "exclamationmark.triangle")
                .fixedSize(horizontal: false, vertical: true)
            if let detail = problem.detail {
                Text(detail)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Button("Retry", action: retry)
                .disabled(retrying)
        }
    }
}

private struct SectionTitle: View {
    let title: String

    init(_ title: String) {
        self.title = title
    }

    var body: some View {
        Text(title).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
    }
}

private struct NotAvailable: View {
    var body: some View {
        Text("Not available").foregroundStyle(.secondary)
    }
}

/// Calls `onOpen` with the panel's window each time it becomes key, i.e. every
/// time the menu bar icon is clicked open. (`onAppear` may only fire the first time.)
private struct PanelOpenObserver: NSViewRepresentable {
    let onOpen: (NSWindow) -> Void

    func makeNSView(context: Context) -> ObserverView {
        let view = ObserverView()
        view.onOpen = onOpen
        return view
    }

    func updateNSView(_ view: ObserverView, context: Context) {
        view.onOpen = onOpen
    }

    final class ObserverView: NSView {
        var onOpen: ((NSWindow) -> Void)?
        private var observer: NSObjectProtocol?

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if let observer { NotificationCenter.default.removeObserver(observer) }
            observer = nil
            guard let window else { return }
            observer = NotificationCenter.default.addObserver(
                forName: NSWindow.didBecomeKeyNotification, object: window, queue: .main
            ) { [weak self, weak window] _ in
                MainActor.assumeIsolated {
                    if let window { self?.onOpen?(window) }
                }
            }
        }
    }
}
