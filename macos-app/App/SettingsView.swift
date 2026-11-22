import AppKit
import MonitorControlCore
import SwiftUI

/// The Settings window: input names, the menu bar icon, Launch at login, and
/// how to add the Control Center controls. Kept around once created, so
/// reopening it is instant.
@MainActor
final class SettingsWindow {
    private let model: StatusModel
    private lazy var window: NSWindow = {
        let window = NSWindow(contentViewController: NSHostingController(rootView: SettingsView(model: model)))
        window.title = "Monitor Control Settings"
        window.styleMask = [.titled, .closable, .miniaturizable]
        window.isReleasedWhenClosed = false
        window.setFrameAutosaveName("Settings")
        if !window.setFrameUsingName("Settings") { window.center() }
        return window
    }()

    init(model: StatusModel) {
        self.model = model
    }

    func show() {
        model.refresh()
        NSApp.activate()
        window.makeKeyAndOrderFront(nil)
    }
}

struct SettingsView: View {
    let model: StatusModel
    @AppStorage(Preferences.showMenuBarIcon) private var showMenuBarIcon = false

    var body: some View {
        Form {
            Section {
                InputNames(model: model)
            } header: {
                Text("Input Names")
            } footer: {
                Text("Custom names appear in Control Center, the menu bar panel, and Shortcuts.")
                    .settingsFootnote()
            }

            Section("Control Center") {
                Text(
                    "Click Control Center in the menu bar, choose **Edit Controls**, and search for **Monitor Control** to add its brightness and input controls. You can also drag a control into the menu bar."
                )
                .fixedSize(horizontal: false, vertical: true)
            }

            Section("App") {
                Toggle("Show menu bar icon", isOn: $showMenuBarIcon)
                LaunchAtLogin(model: model)
            }

            Section {
                HStack {
                    Text("Version \(Self.version)").foregroundStyle(.secondary)
                    Spacer()
                    Button("Quit Monitor Control") { NSApplication.shared.terminate(nil) }
                }
            }
        }
        .formStyle(.grouped)
        .frame(width: 440)
        .fixedSize(horizontal: false, vertical: true)
    }

    private static let version =
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "dev"
}

/// One name field per input, or why there are none.
private struct InputNames: View {
    let model: StatusModel

    var body: some View {
        switch model.state {
        case .loading:
            ProgressView().controlSize(.small)
        case .ready(let status):
            if let inputs = status.inputs {
                LabeledContent("Monitor", value: status.name)
                ForEach(inputs, id: \.code) { input in
                    NameField(model: model, input: input)
                }
            } else {
                Text("\(status.name) doesn't report its inputs.").foregroundStyle(.secondary)
            }
        case .problem(let problem):
            // Same message as the panels; names can still be set later.
            Text(problem.title).foregroundStyle(.secondary)
        }
    }
}

/// A name for one input. Saves when Return is pressed or the field loses focus;
/// a rejected name shows the reason and the field goes back to the saved name.
private struct NameField: View {
    let model: StatusModel
    let input: InputEntry
    @State private var text: String
    @FocusState private var focused: Bool

    init(model: StatusModel, input: InputEntry) {
        self.model = model
        self.input = input
        _text = State(initialValue: input.label ?? "")
    }

    var body: some View {
        VStack(alignment: .trailing, spacing: 4) {
            LabeledContent(input.builtInLabel) {
                // The built-in name is the placeholder, hidden while typing.
                TextField("", text: $text, prompt: focused ? Text("") : Text(input.builtInLabel))
                    .accessibilityLabel(input.builtInLabel)
                    .labelsHidden()
                    .textFieldStyle(.roundedBorder)
                    .multilineTextAlignment(.leading)
                    .frame(width: 200)
                    .focused($focused)
                    .onSubmit(save)
            }
            if let error = model.nameErrors[input.code] {
                Text(error)
                    .font(.callout)
                    .foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .onChange(of: focused) { _, isFocused in
            if !isFocused { save() }
        }
        // Follow names changed elsewhere (the panel, the CLI) unless mid-edit.
        .onChange(of: input.label) { _, label in
            if !focused { text = label ?? "" }
        }
        .onChange(of: model.nameErrors[input.code]) { _, error in
            if error != nil { text = input.label ?? "" }
        }
    }

    private func save() {
        model.saveName(input, to: text)
    }
}

private struct LaunchAtLogin: View {
    let model: StatusModel

    var body: some View {
        Toggle(
            "Launch at login",
            isOn: Binding(get: { model.loginItem == .on }, set: model.setLaunchAtLogin)
        )
        if model.loginItem == .needsApproval {
            Button("Allow in Login Items…", action: LoginItem.openSettings)
                .buttonStyle(.link)
        }
        if let error = model.loginItemError {
            Text(error).foregroundStyle(.red).fixedSize(horizontal: false, vertical: true)
        } else {
            Text("Keeps the Control Center controls quick to respond.").settingsFootnote()
        }
    }
}

extension View {
    fileprivate func settingsFootnote() -> some View {
        font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
    }
}
