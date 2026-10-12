import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io

ShellRoot {
    id: root

    // Load FontAwesome
    FontLoader {
        id: faFont
        source: "file:///usr/share/fonts/OTF/FontAwesome.otf"
    }

    // FontAwesome icon constants (strict NO EMOJIS)
    QtObject {
        id: fa
        readonly property string server: "\uf233"
        readonly property string database: "\uf1c0"
        readonly property string branch: "\uf126"
        readonly property string merge: "\uf0e8"
        readonly property string camera: "\uf030"
        readonly property string history: "\uf1da"
        readonly property string play: "\uf04b"
        readonly property string stop: "\uf04d"
        readonly property string trash: "\uf1f8"
        readonly property string network: "\uf1e0"
        readonly property string sync: "\uf021"
        readonly property string terminal: "\uf120"
        readonly property string plus: "\uf067"
        readonly property string search: "\uf002"
        readonly property string times: "\uf00d"
        readonly property string check: "\uf00c"
        readonly property string archive: "\uf187"
        readonly property string arrowRight: "\uf061"
        readonly property string cpu: "\uf2db"
        readonly property string tag: "\uf02b"
    }

    // Color palette (Dark Theme)
    QtObject {
        id: theme
        readonly property color bg: "#181825"
        readonly property color sidebarBg: "#11111b"
        readonly property color cardBg: "#1e1e2e"
        readonly property color cardHover: "#28283d"
        readonly property color accent: "#89b4fa"
        readonly property color accentHover: "#b4befe"
        readonly property color running: "#a6e3a1"
        readonly property color stopped: "#f38ba8"
        readonly property color warning: "#f9e2af"
        readonly property color textPrimary: "#cdd6f4"
        readonly property color textSecondary: "#a6adc8"
        readonly property color textMuted: "#6c7086"
        readonly property color border: "#313244"
        readonly property color inputBg: "#11111b"
    }

    // State management
    property string d4cBin: Quickshell.env("D4C_BIN") ? Quickshell.env("D4C_BIN") : "/home/mepper/Projects/d4c/target/release/d4c"
    property var servicesList: []
    property var networksList: []
    property var snapshotsList: []
    property string activeTab: "services" // "services", "branching", "networks", "logs"
    property string selectedServiceForSnapshots: ""
    property string selectedServiceForLogs: ""
    property string logContent: ""
    property string statusMessage: "Ready"

    // Command runner helper
    function runD4c(args, callback) {
        statusMessage = "Executing d4c " + args.join(" ") + "...";
        cmdProc.callback = callback;
        cmdProc.command = [root.d4cBin].concat(args);
        cmdProc.stdoutBuffer = "";
        cmdProc.running = true;
    }

    Process {
        id: cmdProc
        property var callback: null
        property string stdoutBuffer: ""

        stdout: SplitParser {
            onRead: data => {
                cmdProc.stdoutBuffer += data + "\n";
            }
        }

        onExited: (code, status) => {
            if (callback) {
                callback(code, cmdProc.stdoutBuffer);
            }
            refreshData();
        }
    }

    // Fetch services list
    function refreshData() {
        psProc.stdoutBuffer = "";
        psProc.command = [root.d4cBin, "ps", "--json"];
        psProc.running = true;
        netProc.stdoutBuffer = "";
        netProc.command = [root.d4cBin, "net", "list", "--json"];
        netProc.running = true;
        if (selectedServiceForSnapshots !== "") {
            fetchSnapshots(selectedServiceForSnapshots);
        }
    }

    Process {
        id: psProc
        command: [root.d4cBin, "ps", "--json"]
        property string stdoutBuffer: ""

        stdout: SplitParser {
            onRead: data => {
                psProc.stdoutBuffer += data + "\n";
            }
        }

        onExited: (code, status) => {
            if (code === 0 && psProc.stdoutBuffer.trim().length > 0) {
                try {
                    root.servicesList = JSON.parse(psProc.stdoutBuffer.trim());
                    if (root.selectedServiceForSnapshots === "" && root.servicesList.length > 0) {
                        root.selectedServiceForSnapshots = root.servicesList[0].name;
                        fetchSnapshots(root.selectedServiceForSnapshots);
                    }
                    if (root.selectedServiceForLogs === "" && root.servicesList.length > 0) {
                        root.selectedServiceForLogs = root.servicesList[0].name;
                    }
                } catch (e) {
                    console.log("Failed to parse services JSON: " + e);
                }
            }
        }
    }

    // Fetch networks list
    Process {
        id: netProc
        command: [root.d4cBin, "net", "list", "--json"]
        property string stdoutBuffer: ""

        stdout: SplitParser {
            onRead: data => {
                netProc.stdoutBuffer += data + "\n";
            }
        }

        onExited: (code, status) => {
            if (code === 0 && netProc.stdoutBuffer.trim().length > 0) {
                try {
                    root.networksList = JSON.parse(netProc.stdoutBuffer.trim());
                } catch (e) {
                    console.log("Failed to parse networks JSON: " + e);
                }
            }
        }
    }

    // Fetch snapshots for a specific service
    function fetchSnapshots(serviceName) {
        snapProc.service = serviceName;
        snapProc.stdoutBuffer = "";
        snapProc.command = [root.d4cBin, "snap", "list", serviceName, "--json"];
        snapProc.running = true;
    }

    Process {
        id: snapProc
        property string service: ""
        property string stdoutBuffer: ""

        stdout: SplitParser {
            onRead: data => {
                snapProc.stdoutBuffer += data + "\n";
            }
        }

        onExited: (code, status) => {
            if (code === 0 && snapProc.stdoutBuffer.trim().length > 0) {
                try {
                    root.snapshotsList = JSON.parse(snapProc.stdoutBuffer.trim());
                } catch (e) {
                    root.snapshotsList = [];
                }
            } else {
                root.snapshotsList = [];
            }
        }
    }

    // Live Log Streaming Process
    Process {
        id: logProc
        property string targetService: ""

        stdout: SplitParser {
            onRead: data => {
                root.logContent += data + "\n";
            }
        }
        stderr: SplitParser {
            onRead: data => {
                root.logContent += "[ERR] " + data + "\n";
            }
        }
    }

    function attachLogs(serviceName) {
        root.selectedServiceForLogs = serviceName;
        root.logContent = "Attaching to logs for " + serviceName + "...\n";
        logProc.running = false;
        logProc.command = [root.d4cBin, "logs", serviceName, "--tail", "100", "-f"];
        logProc.running = true;
        root.activeTab = "logs";
    }

    Component.onCompleted: {
        refreshData();
    }

    FloatingWindow {
        id: mainWindow
        title: "d4c Orchestrator"
        implicitWidth: 1100
        implicitHeight: 720
        visible: true

        Rectangle {
            anchors.fill: parent
            color: theme.bg

            RowLayout {
                anchors.fill: parent
                spacing: 0

                // ==================== SIDEBAR ====================
                Rectangle {
                    Layout.fillHeight: true
                    Layout.preferredWidth: 240
                    color: theme.sidebarBg

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 16
                        spacing: 12

                        // Brand Header
                        RowLayout {
                            spacing: 10
                            Text {
                                text: fa.server
                                font.family: faFont.name
                                font.pixelSize: 22
                                color: theme.accent
                            }
                            ColumnLayout {
                                spacing: 2
                                Text {
                                    text: "d4c"
                                    font.bold: true
                                    font.pixelSize: 18
                                    color: theme.textPrimary
                                }
                                Text {
                                    text: "Podman & Btrfs CoW"
                                    font.pixelSize: 11
                                    color: theme.textMuted
                                }
                            }
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            height: 1
                            color: theme.border
                            Layout.topMargin: 6
                            Layout.bottomMargin: 6
                        }

                        // Navigation Tabs
                        Repeater {
                            model: [
                                { id: "services", title: "Services", icon: fa.server },
                                { id: "branching", title: "Branch & Merge", icon: fa.branch },
                                { id: "networks", title: "Networks", icon: fa.network },
                                { id: "logs", title: "Container Logs", icon: fa.terminal }
                            ]

                            delegate: Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 42
                                radius: 8
                                color: root.activeTab === modelData.id ? theme.cardHover : "transparent"

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    spacing: 12

                                    Text {
                                        text: modelData.icon
                                        font.family: faFont.name
                                        font.pixelSize: 15
                                        color: root.activeTab === modelData.id ? theme.accent : theme.textSecondary
                                    }
                                    Text {
                                        text: modelData.title
                                        font.bold: root.activeTab === modelData.id
                                        font.pixelSize: 13
                                        color: root.activeTab === modelData.id ? theme.textPrimary : theme.textSecondary
                                        Layout.fillWidth: true
                                    }
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: {
                                        root.activeTab = modelData.id;
                                        if (modelData.id === "branching" && root.selectedServiceForSnapshots !== "") {
                                            fetchSnapshots(root.selectedServiceForSnapshots);
                                        }
                                    }
                                }
                            }
                        }

                        Item { Layout.fillHeight: true }

                        // Refresh button
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 38
                            radius: 8
                            color: theme.cardBg

                            RowLayout {
                                anchors.centerIn: parent
                                spacing: 8
                                Text {
                                    text: fa.sync
                                    font.family: faFont.name
                                    font.pixelSize: 13
                                    color: theme.textSecondary
                                }
                                Text {
                                    text: "Refresh Data"
                                    font.pixelSize: 12
                                    color: theme.textPrimary
                                }
                            }

                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: refreshData()
                            }
                        }
                    }
                }

                // ==================== MAIN CONTENT AREA ====================
                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    color: theme.bg

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 24
                        spacing: 16

                        // Top Action Bar
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12

                            Text {
                                text: {
                                    if (root.activeTab === "services") return "Managed Services";
                                    if (root.activeTab === "branching") return "Btrfs Snapshots, Branches & Merges";
                                    if (root.activeTab === "networks") return "Bridge Networks Topology";
                                    return "Live Container Logs";
                                }
                                font.bold: true
                                font.pixelSize: 20
                                color: theme.textPrimary
                                Layout.fillWidth: true
                            }

                            // Add Service Button (shown on services tab)
                            Rectangle {
                                visible: root.activeTab === "services"
                                Layout.preferredWidth: 140
                                Layout.preferredHeight: 36
                                radius: 6
                                color: theme.accent

                                RowLayout {
                                    anchors.centerIn: parent
                                    spacing: 6
                                    Text {
                                        text: fa.plus
                                        font.family: faFont.name
                                        font.pixelSize: 12
                                        color: "#11111b"
                                    }
                                    Text {
                                        text: "Provision Service"
                                        font.bold: true
                                        font.pixelSize: 12
                                        color: "#11111b"
                                    }
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: provisionModal.open()
                                }
                            }
                        }

                        // ----------------- TAB 1: SERVICES LIST -----------------
                        ScrollView {
                            visible: root.activeTab === "services"
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            clip: true

                            ListView {
                                spacing: 10
                                model: root.servicesList

                                delegate: Rectangle {
                                    width: ListView.view.width
                                    height: 80
                                    radius: 10
                                    color: theme.cardBg
                                    border.color: theme.border
                                    border.width: 1

                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.margins: 14
                                        spacing: 16

                                        // Status Dot
                                        Rectangle {
                                            width: 10
                                            height: 10
                                            radius: 5
                                            color: modelData.status.toLowerCase().indexOf("running") !== -1 || modelData.status.toLowerCase().indexOf("up") !== -1 ? theme.running : theme.stopped
                                        }

                                        // Name & Details
                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 4

                                            RowLayout {
                                                spacing: 8
                                                Text {
                                                    text: modelData.name
                                                    font.bold: true
                                                    font.pixelSize: 15
                                                    color: theme.textPrimary
                                                }
                                                Rectangle {
                                                    radius: 4
                                                    color: theme.sidebarBg
                                                    Layout.preferredWidth: catText.implicitWidth + 10
                                                    Layout.preferredHeight: 18
                                                    Text {
                                                        id: catText
                                                        anchors.centerIn: parent
                                                        text: modelData.catalog
                                                        font.pixelSize: 10
                                                        color: theme.accent
                                                    }
                                                }
                                                Text {
                                                    visible: modelData.tag !== null && modelData.tag !== ""
                                                    text: "[" + modelData.tag + "]"
                                                    font.pixelSize: 11
                                                    color: theme.warning
                                                }
                                            }

                                            RowLayout {
                                                spacing: 12
                                                Text {
                                                    text: "Port: " + (modelData.host_port ? modelData.host_port + " -> " + modelData.container_port : "-" + " -> " + modelData.container_port)
                                                    font.pixelSize: 11
                                                    color: theme.textSecondary
                                                }
                                                Text {
                                                    text: "Net: " + (modelData.network ? modelData.network : "default")
                                                    font.pixelSize: 11
                                                    color: theme.textMuted
                                                }
                                                Text {
                                                    text: "Btrfs: " + modelData.volume_path
                                                    font.pixelSize: 10
                                                    color: theme.textMuted
                                                    elide: Text.ElideMiddle
                                                    Layout.maximumWidth: 320
                                                }
                                            }
                                        }

                                        // Action Buttons
                                        RowLayout {
                                            spacing: 6

                                            // Start / Stop Button
                                            Rectangle {
                                                Layout.preferredWidth: 32
                                                Layout.preferredHeight: 32
                                                radius: 6
                                                color: theme.sidebarBg

                                                Text {
                                                    anchors.centerIn: parent
                                                    text: modelData.status.toLowerCase().indexOf("running") !== -1 ? fa.stop : fa.play
                                                    font.family: faFont.name
                                                    font.pixelSize: 12
                                                    color: modelData.status.toLowerCase().indexOf("running") !== -1 ? theme.stopped : theme.running
                                                }

                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        if (modelData.status.toLowerCase().indexOf("running") !== -1) {
                                                            runD4c(["stop", modelData.name], () => {});
                                                        } else {
                                                            runD4c(["start", modelData.name], () => {});
                                                        }
                                                    }
                                                }
                                            }

                                            // Branch Button
                                            Rectangle {
                                                Layout.preferredWidth: 32
                                                Layout.preferredHeight: 32
                                                radius: 6
                                                color: theme.sidebarBg

                                                Text {
                                                    anchors.centerIn: parent
                                                    text: fa.branch
                                                    font.family: faFont.name
                                                    font.pixelSize: 12
                                                    color: theme.accent
                                                }

                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        branchModal.serviceName = modelData.name;
                                                        branchModal.open();
                                                    }
                                                }
                                            }

                                            // Snapshot Button
                                            Rectangle {
                                                Layout.preferredWidth: 32
                                                Layout.preferredHeight: 32
                                                radius: 6
                                                color: theme.sidebarBg

                                                Text {
                                                    anchors.centerIn: parent
                                                    text: fa.camera
                                                    font.family: faFont.name
                                                    font.pixelSize: 12
                                                    color: theme.warning
                                                }

                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        snapshotModal.serviceName = modelData.name;
                                                        snapshotModal.open();
                                                    }
                                                }
                                            }

                                            // Logs Button
                                            Rectangle {
                                                Layout.preferredWidth: 32
                                                Layout.preferredHeight: 32
                                                radius: 6
                                                color: theme.sidebarBg

                                                Text {
                                                    anchors.centerIn: parent
                                                    text: fa.terminal
                                                    font.family: faFont.name
                                                    font.pixelSize: 12
                                                    color: theme.textSecondary
                                                }

                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: attachLogs(modelData.name)
                                                }
                                            }

                                            // Wipe Button
                                            Rectangle {
                                                Layout.preferredWidth: 32
                                                Layout.preferredHeight: 32
                                                radius: 6
                                                color: theme.sidebarBg

                                                Text {
                                                    anchors.centerIn: parent
                                                    text: fa.trash
                                                    font.family: faFont.name
                                                    font.pixelSize: 12
                                                    color: theme.stopped
                                                }

                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        wipeModal.serviceName = modelData.name;
                                                        wipeModal.open();
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // ----------------- TAB 2: BRANCH & MERGE / SNAPSHOTS -----------------
                        ColumnLayout {
                            visible: root.activeTab === "branching"
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            spacing: 14

                            // Top service select and actions
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 12

                                Text {
                                    text: "Target Service:"
                                    color: theme.textSecondary
                                    font.pixelSize: 13
                                }

                                ComboBox {
                                    id: svcSelect
                                    Layout.preferredWidth: 180
                                    model: root.servicesList.map(s => s.name)
                                    onCurrentTextChanged: {
                                        if (currentText) {
                                            root.selectedServiceForSnapshots = currentText;
                                            fetchSnapshots(currentText);
                                        }
                                    }
                                }

                                Item { Layout.fillWidth: true }

                                // Quick Branch Button
                                Rectangle {
                                    Layout.preferredWidth: 120
                                    Layout.preferredHeight: 34
                                    radius: 6
                                    color: theme.accent

                                    RowLayout {
                                        anchors.centerIn: parent
                                        spacing: 6
                                        Text { text: fa.branch; font.family: faFont.name; font.pixelSize: 12; color: "#11111b" }
                                        Text { text: "Branch Volume"; font.bold: true; font.pixelSize: 12; color: "#11111b" }
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            branchModal.serviceName = svcSelect.currentText;
                                            branchModal.open();
                                        }
                                    }
                                }

                                // Merge Branch Button
                                Rectangle {
                                    Layout.preferredWidth: 120
                                    Layout.preferredHeight: 34
                                    radius: 6
                                    color: theme.warning

                                    RowLayout {
                                        anchors.centerIn: parent
                                        spacing: 6
                                        Text { text: fa.merge; font.family: faFont.name; font.pixelSize: 12; color: "#11111b" }
                                        Text { text: "Merge Branch"; font.bold: true; font.pixelSize: 12; color: "#11111b" }
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: mergeModal.open()
                                    }
                                }
                            }

                            // Snapshots Table
                            ScrollView {
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                clip: true

                                ListView {
                                    spacing: 8
                                    model: root.snapshotsList

                                    delegate: Rectangle {
                                        width: ListView.view.width
                                        height: 56
                                        radius: 8
                                        color: theme.cardBg
                                        border.color: theme.border

                                        RowLayout {
                                            anchors.fill: parent
                                            anchors.margins: 12
                                            spacing: 16

                                            Text {
                                                text: fa.camera
                                                font.family: faFont.name
                                                font.pixelSize: 14
                                                color: theme.accent
                                            }

                                            ColumnLayout {
                                                Layout.fillWidth: true
                                                spacing: 2
                                                RowLayout {
                                                    spacing: 8
                                                    Text {
                                                        text: modelData.tag
                                                        font.bold: true
                                                        font.pixelSize: 13
                                                        color: theme.textPrimary
                                                    }
                                                    Text {
                                                        text: modelData.readonly ? "[RO]" : "[RW]"
                                                        font.pixelSize: 10
                                                        color: modelData.readonly ? theme.warning : theme.running
                                                    }
                                                    Text {
                                                        text: (modelData.size_bytes / 1048576).toFixed(2) + " MB"
                                                        font.pixelSize: 11
                                                        color: theme.textSecondary
                                                    }
                                                }
                                                Text {
                                                    text: modelData.created_at ? modelData.created_at : "Path: " + modelData.path
                                                    font.pixelSize: 10
                                                    color: theme.textMuted
                                                }
                                            }

                                            // Restore Button
                                            Rectangle {
                                                Layout.preferredWidth: 70
                                                Layout.preferredHeight: 28
                                                radius: 4
                                                color: theme.sidebarBg

                                                RowLayout {
                                                    anchors.centerIn: parent
                                                    spacing: 4
                                                    Text { text: fa.history; font.family: faFont.name; font.pixelSize: 10; color: theme.running }
                                                    Text { text: "Restore"; font.pixelSize: 11; color: theme.textPrimary }
                                                }

                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        runD4c(["snap", "restore", svcSelect.currentText, "-s", modelData.tag], () => {});
                                                    }
                                                }
                                            }

                                            // Export Button
                                            Rectangle {
                                                Layout.preferredWidth: 70
                                                Layout.preferredHeight: 28
                                                radius: 4
                                                color: theme.sidebarBg

                                                RowLayout {
                                                    anchors.centerIn: parent
                                                    spacing: 4
                                                    Text { text: fa.archive; font.family: faFont.name; font.pixelSize: 10; color: theme.accent }
                                                    Text { text: "Export"; font.pixelSize: 11; color: theme.textPrimary }
                                                }

                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        runD4c(["snap", "export", svcSelect.currentText], () => {});
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // ----------------- TAB 3: NETWORKS -----------------
                        ColumnLayout {
                            visible: root.activeTab === "networks"
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            spacing: 12

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10

                                TextField {
                                    id: newNetName
                                    placeholderText: "New bridge network name (e.g. backend-mesh)"
                                    Layout.fillWidth: true
                                }
                                Rectangle {
                                    Layout.preferredWidth: 120
                                    Layout.preferredHeight: 34
                                    radius: 6
                                    color: theme.accent

                                    RowLayout {
                                        anchors.centerIn: parent
                                        spacing: 6
                                        Text { text: fa.plus; font.family: faFont.name; font.pixelSize: 11; color: "#11111b" }
                                        Text { text: "Create Net"; font.bold: true; font.pixelSize: 12; color: "#11111b" }
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            if (newNetName.text.trim()) {
                                                runD4c(["net", "create", newNetName.text.trim()], () => {
                                                    newNetName.text = "";
                                                });
                                            }
                                        }
                                    }
                                }
                            }

                            ScrollView {
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                clip: true

                                ListView {
                                    spacing: 8
                                    model: root.networksList

                                    delegate: Rectangle {
                                        width: ListView.view.width
                                        height: 60
                                        radius: 8
                                        color: theme.cardBg
                                        border.color: theme.border

                                        RowLayout {
                                            anchors.fill: parent
                                            anchors.margins: 12
                                            spacing: 14

                                            Text {
                                                text: fa.network
                                                font.family: faFont.name
                                                font.pixelSize: 15
                                                color: theme.accent
                                            }

                                            ColumnLayout {
                                                Layout.fillWidth: true
                                                spacing: 2
                                                Text {
                                                    text: modelData.name
                                                    font.bold: true
                                                    font.pixelSize: 13
                                                    color: theme.textPrimary
                                                }
                                                Text {
                                                    text: "Subnet: " + (modelData.subnet ? modelData.subnet : "-") + " | Gateway: " + (modelData.gateway ? modelData.gateway : "-") + " | Containers: " + modelData.containers_count
                                                    font.pixelSize: 11
                                                    color: theme.textSecondary
                                                }
                                            }

                                            Rectangle {
                                                Layout.preferredWidth: 32
                                                Layout.preferredHeight: 32
                                                radius: 6
                                                color: theme.sidebarBg

                                                Text {
                                                    anchors.centerIn: parent
                                                    text: fa.trash
                                                    font.family: faFont.name
                                                    font.pixelSize: 12
                                                    color: theme.stopped
                                                }

                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        runD4c(["net", "rm", modelData.name], () => {});
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // ----------------- TAB 4: CONTAINER LOGS -----------------
                        ColumnLayout {
                            visible: root.activeTab === "logs"
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            spacing: 10

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10

                                Text {
                                    text: "Service:"
                                    color: theme.textSecondary
                                    font.pixelSize: 13
                                }

                                ComboBox {
                                    id: logSvcSelect
                                    Layout.preferredWidth: 180
                                    model: root.servicesList.map(s => s.name)
                                    onCurrentTextChanged: {
                                        if (currentText) {
                                            attachLogs(currentText);
                                        }
                                    }
                                }

                                Item { Layout.fillWidth: true }

                                Rectangle {
                                    Layout.preferredWidth: 80
                                    Layout.preferredHeight: 30
                                    radius: 6
                                    color: theme.sidebarBg

                                    Text {
                                        anchors.centerIn: parent
                                        text: "Clear"
                                        font.pixelSize: 11
                                        color: theme.textSecondary
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: root.logContent = ""
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                radius: 8
                                color: "#0d0e15"
                                border.color: theme.border

                                ScrollView {
                                    anchors.fill: parent
                                    anchors.margins: 10
                                    clip: true

                                    TextArea {
                                        text: root.logContent
                                        readOnly: true
                                        font.family: "monospace"
                                        font.pixelSize: 11
                                        color: theme.textPrimary
                                        background: null
                                        wrapMode: Text.WrapAnywhere
                                    }
                                }
                            }
                        }

                        // Status Bar at Bottom
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 24
                            color: "transparent"

                            RowLayout {
                                anchors.fill: parent
                                Text {
                                    text: root.statusMessage
                                    font.pixelSize: 11
                                    color: theme.textMuted
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: "Backend: Podman & Btrfs | UI: Quickshell"
                                    font.pixelSize: 10
                                    color: theme.textMuted
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ==================== MODALS / DIALOGS ====================

    // Provision Modal
    Dialog {
        id: provisionModal
        title: "Provision Catalog Service"
        anchors.centerIn: parent
        modal: true
        width: 420

        ColumnLayout {
            spacing: 12
            width: parent.width

            Text { text: "Service Catalog:"; color: theme.textSecondary; font.pixelSize: 12 }
            ComboBox {
                id: provCatalog
                Layout.fillWidth: true
                model: ["postgres", "mysql", "mongo", "redis", "kafka", "rabbitmq", "custom"]
            }

            Text { text: "Container Name (optional):"; color: theme.textSecondary; font.pixelSize: 12 }
            TextField { id: provName; Layout.fillWidth: true; placeholderText: "defaults to service type" }

            Text { text: "Host Port (optional, auto-assigned if blank):"; color: theme.textSecondary; font.pixelSize: 12 }
            TextField { id: provPort; Layout.fillWidth: true; placeholderText: "e.g. 5432" }

            Text { text: "Project Tag (optional):"; color: theme.textSecondary; font.pixelSize: 12 }
            TextField { id: provTag; Layout.fillWidth: true; placeholderText: "e.g. dev-cluster" }

            RowLayout {
                Layout.topMargin: 10
                Item { Layout.fillWidth: true }
                Button {
                    text: "Cancel"
                    onClicked: provisionModal.close()
                }
                Button {
                    text: "Provision"
                    onClicked: {
                        var args = ["up", provCatalog.currentText];
                        if (provName.text.trim()) args.push("--name", provName.text.trim());
                        if (provPort.text.trim()) args.push("--port", provPort.text.trim());
                        if (provTag.text.trim()) args.push("--tag", provTag.text.trim());
                        runD4c(args, () => {});
                        provisionModal.close();
                    }
                }
            }
        }
    }

    // Branch Modal
    Dialog {
        id: branchModal
        property string serviceName: ""
        title: "Branch Service: " + serviceName
        anchors.centerIn: parent
        modal: true
        width: 380

        ColumnLayout {
            spacing: 12
            width: parent.width

            Text {
                text: "Create an atomic CoW clone of " + branchModal.serviceName
                color: theme.textSecondary
                font.pixelSize: 12
            }

            Text { text: "Branch Suffix Tag:"; color: theme.textSecondary; font.pixelSize: 12 }
            TextField { id: branchSuffix; Layout.fillWidth: true; placeholderText: "e.g. feat-test, experiment" }

            Text { text: "Dedicated Port (optional):"; color: theme.textSecondary; font.pixelSize: 12 }
            TextField { id: branchPort; Layout.fillWidth: true; placeholderText: "auto-assign if empty" }

            RowLayout {
                Layout.topMargin: 10
                Item { Layout.fillWidth: true }
                Button { text: "Cancel"; onClicked: branchModal.close() }
                Button {
                    text: "Create Branch"
                    onClicked: {
                        if (branchSuffix.text.trim()) {
                            var args = ["snap", "branch", branchModal.serviceName, branchSuffix.text.trim()];
                            if (branchPort.text.trim()) args.push("--port", branchPort.text.trim());
                            runD4c(args, () => {});
                            branchModal.close();
                        }
                    }
                }
            }
        }
    }

    // Snapshot Modal
    Dialog {
        id: snapshotModal
        property string serviceName: ""
        title: "Create Snapshot: " + serviceName
        anchors.centerIn: parent
        modal: true
        width: 360

        ColumnLayout {
            spacing: 12
            width: parent.width

            Text { text: "Snapshot Tag (optional):"; color: theme.textSecondary; font.pixelSize: 12 }
            TextField { id: snapTagInput; Layout.fillWidth: true; placeholderText: "timestamp if empty" }

            CheckBox {
                id: snapRoCheck
                text: "Read-only snapshot"
                checked: true
            }

            RowLayout {
                Layout.topMargin: 10
                Item { Layout.fillWidth: true }
                Button { text: "Cancel"; onClicked: snapshotModal.close() }
                Button {
                    text: "Create"
                    onClicked: {
                        var args = ["snap", "create", snapshotModal.serviceName];
                        if (snapTagInput.text.trim()) args.push("--tag", snapTagInput.text.trim());
                        if (snapRoCheck.checked) args.push("--readonly");
                        runD4c(args, () => {});
                        snapshotModal.close();
                    }
                }
            }
        }
    }

    // Merge Modal
    Dialog {
        id: mergeModal
        title: "Merge Branch into Target Service"
        anchors.centerIn: parent
        modal: true
        width: 400

        ColumnLayout {
            spacing: 12
            width: parent.width

            Text { text: "Source Branch Service:"; color: theme.textSecondary; font.pixelSize: 12 }
            ComboBox {
                id: mergeSrc
                Layout.fillWidth: true
                model: root.servicesList.map(s => s.name)
            }

            Text { text: "Target Service to receive changes:"; color: theme.textSecondary; font.pixelSize: 12 }
            ComboBox {
                id: mergeTgt
                Layout.fillWidth: true
                model: root.servicesList.map(s => s.name)
            }

            Text {
                text: "Note: A safety backup snapshot will be automatically created before merging."
                font.pixelSize: 11
                color: theme.warning
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }

            RowLayout {
                Layout.topMargin: 10
                Item { Layout.fillWidth: true }
                Button { text: "Cancel"; onClicked: mergeModal.close() }
                Button {
                    text: "Execute Merge"
                    onClicked: {
                        if (mergeSrc.currentText && mergeTgt.currentText && mergeSrc.currentText !== mergeTgt.currentText) {
                            runD4c(["snap", "merge", mergeSrc.currentText, mergeTgt.currentText], () => {});
                            mergeModal.close();
                        }
                    }
                }
            }
        }
    }

    // Wipe Modal
    Dialog {
        id: wipeModal
        property string serviceName: ""
        title: "Wipe Service: " + serviceName
        anchors.centerIn: parent
        modal: true
        width: 360

        ColumnLayout {
            spacing: 12
            width: parent.width

            Text {
                text: "Destroy container AND all Btrfs volume data & snapshots for " + wipeModal.serviceName + "?"
                color: theme.stopped
                font.bold: true
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }

            RowLayout {
                Layout.topMargin: 10
                Item { Layout.fillWidth: true }
                Button { text: "Cancel"; onClicked: wipeModal.close() }
                Button {
                    text: "Destroy & Wipe"
                    onClicked: {
                        runD4c(["wipe", wipeModal.serviceName, "--force"], () => {});
                        wipeModal.close();
                    }
                }
            }
        }
    }
}
