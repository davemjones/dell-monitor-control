import Foundation
import Testing

@testable import MonitorControlCore

private func finished(_ code: Int32, stdout: String = "", stderr: String = "") -> RunResult {
    .finished(exitCode: code, stdout: Data(stdout.utf8), stderr: Data(stderr.utf8))
}

@Test func labels() {
    let label = { InputEntry(name: $0, code: "", active: false).label }
    #expect(label("thunderbolt") == "Thunderbolt / USB-C")
    #expect(label("dp") == "DisplayPort")
    #expect(label("hdmi") == "HDMI")
    #expect(label("input-0x1b") == "input-0x1b")
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
