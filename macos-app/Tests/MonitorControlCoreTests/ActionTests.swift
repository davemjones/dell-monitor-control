import Foundation
import Testing

@testable import MonitorControlCore

private func finished(_ code: Int32, stdout: String = "", stderr: String = "") -> RunResult {
    .finished(exitCode: code, stdout: Data(stdout.utf8), stderr: Data(stderr.utf8))
}

@Test func labels() {
    let label = { InputEntry(name: $0, code: "", active: false).displayName }
    #expect(label("thunderbolt") == "Thunderbolt / USB-C")
    #expect(label("dp") == "DisplayPort")
    #expect(label("hdmi") == "HDMI")
    #expect(label("input-0x1b") == "input-0x1b")
    let named = InputEntry(name: "hdmi", code: "0x11", label: "Gaming PC", active: false)
    #expect(named.displayName == "Gaming PC")
    #expect(named.builtInLabel == "HDMI")
}

@Test func renameArguments() {
    let hdmi = InputEntry(name: "hdmi", code: "0x11", active: false)
    #expect(hdmi.newLabel(from: "  Gaming PC ") == "Gaming PC")
    #expect(hdmi.newLabel(from: " ") == nil)
    #expect(hdmi.newLabel(from: "HDMI") == nil)
    #expect(hdmi.renameArguments("Gaming PC") == ["input", "name", "0x11", "Gaming PC", "--json"])
    #expect(hdmi.renameArguments(nil) == ["input", "name", "0x11", "--clear", "--json"])
}

@Test func readsRename() {
    let json = """
        {"input":{"name":"hdmi","code":"0x11","label":"Gaming PC"},
         "label":{"before":null,"after":"Gaming PC"}}
        """
    #expect(
        parseRename(finished(0, stdout: json))
            == InputRef(name: "hdmi", code: "0x11", label: "Gaming PC"))
    let cleared = #"{"input":{"name":"hdmi","code":"0x11","label":null},"label":{"before":"X","after":null}}"#
    #expect(parseRename(finished(0, stdout: cleared))?.label == nil)
    #expect(parseRename(finished(1, stderr: "nope")) == nil)
}

@Test func decodesLabelsWhenPresentOrMissing() throws {
    let json = """
        {"monitor":1,"name":"DELL U3425WE",
         "input":{"name":"thunderbolt","code":"0x19","label":"Work Mac"},
         "inputs":[{"name":"dp","code":"0x0f","active":false},
                   {"name":"thunderbolt","code":"0x19","label":"Work Mac","active":true}],
         "brightness":78}
        """
    let status = try JSONDecoder().decode(MonitorStatus.self, from: Data(json.utf8))
    #expect(status.input?.label == "Work Mac")
    #expect(status.inputs?.map(\.label) == [nil, "Work Mac"])
}

@Test func renamedStatusUpdatesTheInput() throws {
    let json = """
        {"monitor":1,"name":"DELL U3425WE","input":{"name":"thunderbolt","code":"0x19"},
         "inputs":[{"name":"dp","code":"0x0f","active":false},
                   {"name":"thunderbolt","code":"0x19","active":true}],
         "brightness":78}
        """
    let status = try JSONDecoder().decode(MonitorStatus.self, from: Data(json.utf8))
    let named = status.renamed(code: "0x19", to: "Work Mac")
    #expect(named.inputs?.map(\.label) == [nil, "Work Mac"])
    #expect(named.inputs?.map(\.active) == [false, true])
    #expect(named.input?.label == "Work Mac")
    // Switching keeps names.
    let switched = named.switched(to: "0x0f")
    #expect(switched.inputs?.map(\.label) == [nil, "Work Mac"])
    #expect(named.renamed(code: "0x19", to: nil).input?.label == nil)
}

@Test func failureMessages() {
    #expect(failureMessage(of: finished(0, stdout: "{}")) == nil)
    let err = #"{"error":{"kind":"failed","message":"couldn't change brightness"}}"#
    #expect(failureMessage(of: finished(1, stderr: err)) == "Couldn't change brightness")
    #expect(failureMessage(of: finished(1, stderr: "error: boom")) == "Error: boom")
    #expect(failureMessage(of: .timedOut) == Problem.timedOut.title)
    #expect(failureMessage(of: .launchFailed) == Problem.cantRunMonitorctl.title)
}

@Test func readsBrightness() {
    #expect(parseBrightness(finished(0, stdout: #"{"brightness": 42}"#)) == 42)
    #expect(parseBrightness(finished(1, stderr: "nope")) == nil)
    #expect(parseBrightness(.timedOut) == nil)
}

@Test func switchedStatusMovesTheCheckmark() throws {
    let json = """
        {"monitor":1,"name":"DELL U3425WE","input":{"name":"thunderbolt","code":"0x19"},
         "inputs":[{"name":"dp","code":"0x0f","active":false},
                   {"name":"thunderbolt","code":"0x19","active":true}],
         "brightness":78}
        """
    let status = try JSONDecoder().decode(MonitorStatus.self, from: Data(json.utf8))
    let after = status.switched(to: "0x0f")
    #expect(after.inputs?.filter(\.active).map(\.name) == ["dp"])
    #expect(after.input?.name == "dp")
}

@Test func writerCoalescesADrag() {
    var writer = BrightnessWriter(current: 50)
    #expect(writer.want(50) == nil)  // already there
    #expect(writer.want(51) == 51)  // first write starts
    for value in 52...99 {
        #expect(writer.want(value) == nil)  // overtaken while 51 is in flight
    }
    #expect(writer.want(100) == nil)
    #expect(writer.finished() == 100)  // the release value always lands
    #expect(writer.finished() == nil)  // idle
    #expect(writer.want(100) == nil)  // no repeat write
}

@Test func writerReportsAWriteInFlight() {
    var writer = BrightnessWriter(current: 50)
    #expect(!writer.isWriting)
    _ = writer.want(60)
    #expect(writer.isWriting)
    _ = writer.want(70)
    #expect(writer.finished() == 70)
    #expect(writer.isWriting)  // the queued value is now in flight
    #expect(writer.finished() == nil)
    #expect(!writer.isWriting)
}

@Test func writerSkipsAPendingValueThatWasAlreadyWritten() {
    var writer = BrightnessWriter(current: 50)
    #expect(writer.want(60) == 60)
    #expect(writer.want(70) == nil)
    #expect(writer.want(60) == nil)  // dragged back before 60 finished
    #expect(writer.finished() == nil)
}

@Test func writerResetsAfterAFailure() {
    var writer = BrightnessWriter(current: 50)
    #expect(writer.want(60) == 60)
    #expect(writer.want(70) == nil)
    writer.failed(current: 50)
    #expect(writer.want(70) == 70)
}

@Test func launchGateOpensSettingsOnlyForAPersonsLaunch() {
    #expect(LaunchGate(isLoginItem: false).showsSettings)
    #expect(!LaunchGate(isLoginItem: true).showsSettings)
    var action = LaunchGate(isLoginItem: false)
    action.noteAction()
    #expect(!action.showsSettings)
}

@Test func panelsOpenTopRightUnderTheMenuBar() {
    // A 1512×982 screen whose visible area starts below a 37pt menu bar.
    let visible = CGRect(x: 0, y: 0, width: 1512, height: 945)
    #expect(panelOrigin(size: CGSize(width: 280, height: 120), visibleFrame: visible)
        == CGPoint(x: 1224, y: 817))
    // A second screen to the right, with its own origin.
    let second = CGRect(x: 1512, y: -200, width: 3440, height: 1415)
    #expect(panelOrigin(size: CGSize(width: 280, height: 120), visibleFrame: second)
        == CGPoint(x: 4664, y: 1087))
    // Never off the left or bottom edge, even if the panel is larger than the screen.
    #expect(panelOrigin(size: CGSize(width: 2000, height: 2000), visibleFrame: visible)
        == CGPoint(x: 0, y: 0))
}

@Test func shortcutsListInputsByTheirNames() throws {
    let json = """
        [{"name":"dp","code":"0x0f","label":null,"active":false},
         {"name":"hdmi","code":"0x11","label":"Gaming PC","active":true}]
        """
    let entities = try parseInputList(finished(0, stdout: json)).map(MonitorInputEntity.init)
    #expect(entities.map(\.id) == ["0x0f", "0x11"])
    #expect(entities.map(\.name) == ["DisplayPort", "Gaming PC"])
    #expect(entities.map(\.builtInName) == ["DisplayPort", "HDMI"])
}

@Test func shortcutsReportMonitorctlErrors() {
    let err = #"{"error":{"kind":"no_monitor","message":"no DDC/CI-capable monitors found"}}"#
    #expect(throws: MonitorctlFailure.self) { try parseInputList(finished(1, stderr: err)) }
    do {
        _ = try parseInputList(finished(1, stderr: err))
    } catch let failure as MonitorctlFailure {
        #expect(failure.message == "No DDC/CI-capable monitors found")
    } catch {
        Issue.record("unexpected \(error)")
    }
    #expect(throws: MonitorctlFailure.self) { try parseInputList(finished(0, stdout: "nope")) }
}
