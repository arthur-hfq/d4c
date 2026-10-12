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

    // FontAwesome icon constants (NO EMOJIS)
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
        readonly property string cpu: "\uf2db"
        readonly property string tag: "\uf02b"
        readonly property string sliders: "\uf1de"
    }

    // Unified Design System Colors
    QtObject {
        id: theme
        readonly property color bg: "#0f1117"
        readonly property color sidebarBg: "#161822"
        readonly property color cardBg: "#1e2130"
        readonly property color cardHover: "#262a3d"
        readonly property color cardBorder: "#2d3248"
        readonly property color modalBg: "#1a1c28"
        
        readonly property color primary: "#4f46e5"
        readonly property color primaryHover: "#6366f1"
        readonly property color primaryText: "#ffffff"
        
        readonly property color running: "#10b981"
        readonly property color runningBg: "#064e3b"
        readonly property color stopped: "#ef4444"
        readonly property color stoppedBg: "#7f1d1d"
        
        readonly property color warning: "#f59e0b"
        readonly property color branchColor: "#a855f7"
        readonly property color textMain: "#f3f4f6"
        readonly property color textSub: "#9ca3af"
        readonly property color textMuted: "#6b7280"
        readonly property color inputBg: "#11131c"
        readonly property color inputBorder: "#374151"
        readonly property color inputFocus: "#6366f1"
    }

    // App state
    property string d4cBin: Quickshell.env("D4C_BIN") ? Quickshell.env("D4C_BIN") : "/home/mepper/Projects/d4c/target/release/d4c"
    property var servicesList: []
    property var networksList: []
    property var snapshotsList: []
    property string activeTab: "services" // "services", "branching", "networks", "logs"
    property string selectedServiceForSnapshots: ""
    property string selectedServiceForLogs: ""
    property string logContent: ""
    property string statusMessage: "Ready"
    property string activeModal: "" // "", "provision", "branch", "snapshot", "merge", "wipe", "updateLimits"
    property var modalTargetService: null

    // Helper: Execute d4c commands asynchronously
    function runD4c(args, callback) {
        statusMessage = "Running: d4c " + args.join(" ") + "...";
        cmdProc.callback = callback;
        cmdProc.command = [root.d4cBin].concat(args);
        cmdProc.stdoutBuffer = "";
        cmdProc.stderrBuffer = "";
        cmdProc.running = true;
    }

    Process {
        id: cmdProc
        property var callback: null
        property string stdoutBuffer: ""
        property string stderrBuffer: ""

        stdout: SplitParser {
            onRead: data => { cmdProc.stdoutBuffer += data + "\n"; }
        }
        stderr: SplitParser {
            onRead: data => { cmdProc.stderrBuffer += data + "\n"; }
        }

        onExited: (code, status) => {
            if (code === 0) {
                root.statusMessage = "[OK] Command executed successfully.";
            } else {
                root.statusMessage = "[ERR] " + (cmdProc.stderrBuffer.trim() ? cmdProc.stderrBuffer.trim() : "Failed with exit code " + code);
            }
            if (callback) {
                callback(code, cmdProc.stdoutBuffer, cmdProc.stderrBuffer);
            }
            refreshData();
        }
    }

    // Refresh services and networks
    function refreshData() {
        psProc.stdoutBuffer = "";
        psProc.command = [root.d4cBin, "ps", "--json"];
        psProc.running = true;

        netProc.stdoutBuffer = "";
        netProc.command = [root.d4cBin, "net", "list", "--json"];
        netProc.running = true;

        if (root.selectedServiceForSnapshots !== "") {
            fetchSnapshots(root.selectedServiceForSnapshots);
        }
    }

    Process {
        id: psProc
        command: [root.d4cBin, "ps", "--json"]
        property string stdoutBuffer: ""

        stdout: SplitParser {
            onRead: data => { psProc.stdoutBuffer += data + "\n"; }
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
                    console.log("Error parsing services: " + e);
                }
            }
        }
    }

    Process {
        id: netProc
        command: [root.d4cBin, "net", "list", "--json"]
        property string stdoutBuffer: ""

        stdout: SplitParser {
            onRead: data => { netProc.stdoutBuffer += data + "\n"; }
        }

        onExited: (code, status) => {
            if (code === 0 && netProc.stdoutBuffer.trim().length > 0) {
                try {
                    root.networksList = JSON.parse(netProc.stdoutBuffer.trim());
                } catch (e) {
                    console.log("Error parsing networks: " + e);
                }
            }
        }
    }

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
            onRead: data => { snapProc.stdoutBuffer += data + "\n"; }
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
        stdout: SplitParser {
            onRead: data => { root.logContent += data + "\n"; }
        }
        stderr: SplitParser {
            onRead: data => { root.logContent += "[ERR] " + data + "\n"; }
        }
    }

    function attachLogs(serviceName) {
        root.selectedServiceForLogs = serviceName;
        root.logContent = "Attaching to live logs for " + serviceName + "...\n";
        logProc.running = false;
        logProc.command = [root.d4cBin, "logs", serviceName, "--tail", "100", "-f"];
        logProc.running = true;
        root.activeTab = "logs";
    }

    Component.onCompleted: {
        refreshData();
    }

    // ==================== MAIN FLOATING WINDOW ====================
    FloatingWindow {
        id: win
        title: "d4c Orchestrator"
        implicitWidth: 1120
        implicitHeight: 740
        visible: true

        Rectangle {
            anchors.fill: parent
            color: theme.bg

            RowLayout {
                anchors.fill: parent
                spacing: 0

                // ----------------- SIDEBAR -----------------
                Rectangle {
                    Layout.fillHeight: true
                    Layout.preferredWidth: 230
                    color: theme.sidebarBg

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 16
                        spacing: 10

                        // App Brand
                        RowLayout {
                            spacing: 10
                            Text {
                                text: fa.server
                                font.family: faFont.name
                                font.pixelSize: 22
                                color: theme.primary
                            }
                            ColumnLayout {
                                spacing: 1
                                Text {
                                    text: "d4c"
                                    font.bold: true
                                    font.pixelSize: 18
                                    color: theme.textMain
                                }
                                Text {
                                    text: "Podman & Btrfs Orchestrator"
                                    font.pixelSize: 10
                                    color: theme.textMuted
                                }
                            }
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            height: 1
                            color: theme.cardBorder
                            Layout.topMargin: 8
                            Layout.bottomMargin: 8
                        }

                        // Navigation Items
                        Repeater {
                            model: [
                                { id: "services", title: "Services", icon: fa.server },
                                { id: "branching", title: "Branch & Merge", icon: fa.branch },
                                { id: "networks", title: "Bridge Networks", icon: fa.network },
                                { id: "logs", title: "Container Logs", icon: fa.terminal }
                            ]

                            delegate: Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 40
                                radius: 6
                                color: root.activeTab === modelData.id ? theme.primary : (navMouse.containsMouse ? theme.cardHover : "transparent")

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    spacing: 12

                                    Text {
                                        text: modelData.icon
                                        font.family: faFont.name
                                        font.pixelSize: 14
                                        color: root.activeTab === modelData.id ? theme.primaryText : (navMouse.containsMouse ? theme.textMain : theme.textSub)
                                    }
                                    Text {
                                        text: modelData.title
                                        font.bold: root.activeTab === modelData.id
                                        font.pixelSize: 13
                                        color: root.activeTab === modelData.id ? theme.primaryText : (navMouse.containsMouse ? theme.textMain : theme.textSub)
                                        Layout.fillWidth: true
                                    }
                                }

                                MouseArea {
                                    id: navMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
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

                        // Refresh Button
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 36
                            radius: 6
                            color: refMouse.containsMouse ? theme.cardHover : theme.cardBg
                            border.color: theme.cardBorder
                            border.width: 1

                            RowLayout {
                                anchors.centerIn: parent
                                spacing: 8
                                Text {
                                    text: fa.sync
                                    font.family: faFont.name
                                    font.pixelSize: 12
                                    color: theme.textSub
                                }
                                Text {
                                    text: "Refresh All"
                                    font.pixelSize: 12
                                    color: theme.textMain
                                }
                            }

                            MouseArea {
                                id: refMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: refreshData()
                            }
                        }
                    }
                }

                // ----------------- MAIN VIEW -----------------
                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    color: theme.bg

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 20
                        spacing: 16

                        // Header Bar
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12

                            Text {
                                text: {
                                    if (root.activeTab === "services") return "Container Services";
                                    if (root.activeTab === "branching") return "Btrfs Snapshots, Branches & Merges";
                                    if (root.activeTab === "networks") return "Bridge Networks Topology";
                                    return "Container Live Logs";
                                }
                                font.bold: true
                                font.pixelSize: 18
                                color: theme.textMain
                                Layout.fillWidth: true
                            }

                            // Provision button on Services tab
                            Rectangle {
                                visible: root.activeTab === "services"
                                Layout.preferredWidth: 140
                                Layout.preferredHeight: 34
                                radius: 6
                                color: provBtnMouse.containsMouse ? theme.primaryHover : theme.primary

                                RowLayout {
                                    anchors.centerIn: parent
                                    spacing: 6
                                    Text { text: fa.plus; font.family: faFont.name; font.pixelSize: 12; color: theme.primaryText }
                                    Text { text: "New Service"; font.bold: true; font.pixelSize: 12; color: theme.primaryText }
                                }

                                MouseArea {
                                    id: provBtnMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: root.activeModal = "provision"
                                }
                            }
                        }

                        // ==================== VIEW 1: SERVICES LIST ====================
                        Item {
                            visible: root.activeTab === "services"
                            Layout.fillWidth: true
                            Layout.fillHeight: true

                            // Empty state
                            ColumnLayout {
                                anchors.centerIn: parent
                                visible: root.servicesList.length === 0
                                spacing: 10
                                Text {
                                    text: fa.server
                                    font.family: faFont.name
                                    font.pixelSize: 36
                                    color: theme.textMuted
                                    Layout.alignment: Qt.AlignHCenter
                                }
                                Text {
                                    text: "No managed services found."
                                    font.pixelSize: 14
                                    color: theme.textSub
                                    Layout.alignment: Qt.AlignHCenter
                                }
                                Text {
                                    text: "Click 'New Service' to provision a preconfigured catalog service."
                                    font.pixelSize: 12
                                    color: theme.textMuted
                                    Layout.alignment: Qt.AlignHCenter
                                }
                            }

                            ListView {
                                id: svcListView
                                anchors.fill: parent
                                spacing: 10
                                clip: true
                                model: root.servicesList
                                boundsBehavior: Flickable.StopAtBounds

                                ScrollBar.vertical: ScrollBar {
                                    active: true
                                    policy: ScrollBar.AsNeeded
                                }

                                delegate: Rectangle {
                                    width: svcListView.width
                                    height: 96
                                    radius: 8
                                    color: theme.cardBg
                                    border.color: theme.cardBorder
                                    border.width: 1

                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.margins: 14
                                        spacing: 16

                                        // Status dot
                                        Rectangle {
                                            width: 12
                                            height: 12
                                            radius: 6
                                            color: modelData.status.toLowerCase().indexOf("running") !== -1 || modelData.status.toLowerCase().indexOf("up") !== -1 ? theme.running : theme.stopped
                                        }

                                        // Service Details Column
                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 5

                                            // Top line: Name + Badges
                                            RowLayout {
                                                spacing: 8
                                                Text {
                                                    text: modelData.name
                                                    font.bold: true
                                                    font.pixelSize: 15
                                                    color: theme.textMain
                                                }
                                                // Catalog Badge
                                                Rectangle {
                                                    radius: 4
                                                    color: theme.sidebarBg
                                                    border.color: theme.cardBorder
                                                    border.width: 1
                                                    Layout.preferredWidth: catTxt.implicitWidth + 12
                                                    Layout.preferredHeight: 20
                                                    Text {
                                                        id: catTxt
                                                        anchors.centerIn: parent
                                                        text: modelData.catalog
                                                        font.pixelSize: 11
                                                        color: theme.primaryHover
                                                    }
                                                }
                                                // Tag Badge
                                                Rectangle {
                                                    visible: modelData.tag !== null && modelData.tag !== ""
                                                    radius: 4
                                                    color: theme.sidebarBg
                                                    Layout.preferredWidth: tagTxt.implicitWidth + 12
                                                    Layout.preferredHeight: 20
                                                    Text {
                                                        id: tagTxt
                                                        anchors.centerIn: parent
                                                        text: modelData.tag ? "[" + modelData.tag + "]" : ""
                                                        font.pixelSize: 10
                                                        color: theme.warning
                                                    }
                                                }
                                                // Resource Limits Badge
                                                Rectangle {
                                                    radius: 4
                                                    color: theme.sidebarBg
                                                    border.color: theme.cardBorder
                                                    border.width: 1
                                                    Layout.preferredWidth: limTxt.implicitWidth + 12
                                                    Layout.preferredHeight: 20
                                                    RowLayout {
                                                        id: limTxt
                                                        anchors.centerIn: parent
                                                        spacing: 4
                                                        Text { text: fa.cpu; font.family: faFont.name; font.pixelSize: 10; color: theme.textSub }
                                                        Text {
                                                            text: {
                                                                var c = modelData.cpus ? "CPU: " + modelData.cpus : "CPU: unltd";
                                                                var m = modelData.memory ? "RAM: " + modelData.memory : "RAM: unltd";
                                                                return c + " | " + m;
                                                            }
                                                            font.pixelSize: 10
                                                            color: theme.textSub
                                                        }
                                                    }
                                                }
                                            }

                                            // Subtitle info row
                                            RowLayout {
                                                spacing: 14
                                                Text {
                                                    text: "Port: " + (modelData.host_port ? modelData.host_port + ":" + modelData.container_port : "-:" + modelData.container_port)
                                                    font.pixelSize: 11
                                                    color: theme.textSub
                                                }
                                                Text {
                                                    text: "Network: " + (modelData.network ? modelData.network : "default")
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

                                        // Action buttons toolbar
                                        RowLayout {
                                            spacing: 6

                                            // Start / Stop Button
                                            Rectangle {
                                                Layout.preferredWidth: 32
                                                Layout.preferredHeight: 32
                                                radius: 6
                                                color: ssMouse.containsMouse ? theme.cardHover : theme.sidebarBg
                                                border.color: theme.cardBorder

                                                Text {
                                                    anchors.centerIn: parent
                                                    text: modelData.status.toLowerCase().indexOf("running") !== -1 ? fa.stop : fa.play
                                                    font.family: faFont.name
                                                    font.pixelSize: 12
                                                    color: modelData.status.toLowerCase().indexOf("running") !== -1 ? theme.stopped : theme.running
                                                }

                                                MouseArea {
                                                    id: ssMouse
                                                    anchors.fill: parent
                                                    hoverEnabled: true
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

                                            // Resource Limits (Tune) Button
                                            Rectangle {
                                                Layout.preferredWidth: 32
                                                Layout.preferredHeight: 32
                                                radius: 6
                                                color: limMouse.containsMouse ? theme.cardHover : theme.sidebarBg
                                                border.color: theme.cardBorder

                                                Text {
                                                    anchors.centerIn: parent
                                                    text: fa.cpu
                                                    font.family: faFont.name
                                                    font.pixelSize: 12
                                                    color: theme.primaryHover
                                                }

                                                MouseArea {
                                                    id: limMouse
                                                    anchors.fill: parent
                                                    hoverEnabled: true
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        root.modalTargetService = modelData;
                                                        limitCpuInput.text = modelData.cpus ? modelData.cpus : "";
                                                        limitMemInput.text = modelData.memory ? modelData.memory : "";
                                                        root.activeModal = "updateLimits";
                                                    }
                                                }
                                            }

                                            // Branch Button
                                            Rectangle {
                                                Layout.preferredWidth: 32
                                                Layout.preferredHeight: 32
                                                radius: 6
                                                color: brMouse.containsMouse ? theme.cardHover : theme.sidebarBg
                                                border.color: theme.cardBorder

                                                Text {
                                                    anchors.centerIn: parent
                                                    text: fa.branch
                                                    font.family: faFont.name
                                                    font.pixelSize: 12
                                                    color: theme.branchColor
                                                }

                                                MouseArea {
                                                    id: brMouse
                                                    anchors.fill: parent
                                                    hoverEnabled: true
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        root.modalTargetService = modelData;
                                                        branchSuffixInput.text = "";
                                                        branchPortInput.text = "";
                                                        branchCpuInput.text = modelData.cpus ? modelData.cpus : "";
                                                        branchMemInput.text = modelData.memory ? modelData.memory : "";
                                                        root.activeModal = "branch";
                                                    }
                                                }
                                            }

                                            // Snapshot Button
                                            Rectangle {
                                                Layout.preferredWidth: 32
                                                Layout.preferredHeight: 32
                                                radius: 6
                                                color: snMouse.containsMouse ? theme.cardHover : theme.sidebarBg
                                                border.color: theme.cardBorder

                                                Text {
                                                    anchors.centerIn: parent
                                                    text: fa.camera
                                                    font.family: faFont.name
                                                    font.pixelSize: 12
                                                    color: theme.warning
                                                }

                                                MouseArea {
                                                    id: snMouse
                                                    anchors.fill: parent
                                                    hoverEnabled: true
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        root.modalTargetService = modelData;
                                                        snapTagInput.text = "";
                                                        root.activeModal = "snapshot";
                                                    }
                                                }
                                            }

                                            // Logs Button
                                            Rectangle {
                                                Layout.preferredWidth: 32
                                                Layout.preferredHeight: 32
                                                radius: 6
                                                color: lgMouse.containsMouse ? theme.cardHover : theme.sidebarBg
                                                border.color: theme.cardBorder

                                                Text {
                                                    anchors.centerIn: parent
                                                    text: fa.terminal
                                                    font.family: faFont.name
                                                    font.pixelSize: 12
                                                    color: theme.textSub
                                                }

                                                MouseArea {
                                                    id: lgMouse
                                                    anchors.fill: parent
                                                    hoverEnabled: true
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: attachLogs(modelData.name)
                                                }
                                            }

                                            // Wipe Button
                                            Rectangle {
                                                Layout.preferredWidth: 32
                                                Layout.preferredHeight: 32
                                                radius: 6
                                                color: wpMouse.containsMouse ? theme.cardHover : theme.sidebarBg
                                                border.color: theme.cardBorder

                                                Text {
                                                    anchors.centerIn: parent
                                                    text: fa.trash
                                                    font.family: faFont.name
                                                    font.pixelSize: 12
                                                    color: theme.stopped
                                                }

                                                MouseArea {
                                                    id: wpMouse
                                                    anchors.fill: parent
                                                    hoverEnabled: true
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        root.modalTargetService = modelData;
                                                        root.activeModal = "wipe";
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // ==================== VIEW 2: BRANCH & MERGE ====================
                        ColumnLayout {
                            visible: root.activeTab === "branching"
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            spacing: 14

                            // Service Selector Bar
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 12

                                Text {
                                    text: "Service:"
                                    color: theme.textSub
                                    font.pixelSize: 13
                                }

                                ComboBox {
                                    id: snapSvcSelector
                                    Layout.preferredWidth: 200
                                    model: root.servicesList.map(s => s.name)
                                    onCurrentTextChanged: {
                                        if (currentText) {
                                            root.selectedServiceForSnapshots = currentText;
                                            fetchSnapshots(currentText);
                                        }
                                    }
                                }

                                Item { Layout.fillWidth: true }

                                // Branch Volume Button
                                Rectangle {
                                    Layout.preferredWidth: 130
                                    Layout.preferredHeight: 34
                                    radius: 6
                                    color: brTopMouse.containsMouse ? "#c084fc" : theme.branchColor

                                    RowLayout {
                                        anchors.centerIn: parent
                                        spacing: 6
                                        Text { text: fa.branch; font.family: faFont.name; font.pixelSize: 12; color: "#ffffff" }
                                        Text { text: "Branch Volume"; font.bold: true; font.pixelSize: 12; color: "#ffffff" }
                                    }

                                    MouseArea {
                                        id: brTopMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            var svc = root.servicesList.find(s => s.name === snapSvcSelector.currentText);
                                            root.modalTargetService = svc ? svc : { name: snapSvcSelector.currentText };
                                            branchSuffixInput.text = "";
                                            branchPortInput.text = "";
                                            branchCpuInput.text = svc && svc.cpus ? svc.cpus : "";
                                            branchMemInput.text = svc && svc.memory ? svc.memory : "";
                                            root.activeModal = "branch";
                                        }
                                    }
                                }

                                // Merge Branch Button
                                Rectangle {
                                    Layout.preferredWidth: 130
                                    Layout.preferredHeight: 34
                                    radius: 6
                                    color: mgTopMouse.containsMouse ? "#fbbf24" : theme.warning

                                    RowLayout {
                                        anchors.centerIn: parent
                                        spacing: 6
                                        Text { text: fa.merge; font.family: faFont.name; font.pixelSize: 12; color: "#11131c" }
                                        Text { text: "Merge Branch"; font.bold: true; font.pixelSize: 12; color: "#11131c" }
                                    }

                                    MouseArea {
                                        id: mgTopMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: root.activeModal = "merge"
                                    }
                                }
                            }

                            // Snapshots Table
                            ListView {
                                id: snapListView
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                clip: true
                                spacing: 8
                                model: root.snapshotsList
                                boundsBehavior: Flickable.StopAtBounds

                                ScrollBar.vertical: ScrollBar {
                                    active: true
                                    policy: ScrollBar.AsNeeded
                                }

                                delegate: Rectangle {
                                    width: snapListView.width
                                    height: 56
                                    radius: 8
                                    color: theme.cardBg
                                    border.color: theme.cardBorder

                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.margins: 12
                                        spacing: 16

                                        Text {
                                            text: fa.camera
                                            font.family: faFont.name
                                            font.pixelSize: 14
                                            color: theme.primaryHover
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
                                                    color: theme.textMain
                                                }
                                                Text {
                                                    text: modelData.readonly ? "[RO]" : "[RW]"
                                                    font.pixelSize: 10
                                                    color: modelData.readonly ? theme.warning : theme.running
                                                }
                                                Text {
                                                    text: (modelData.size_bytes / 1048576).toFixed(2) + " MB"
                                                    font.pixelSize: 11
                                                    color: theme.textSub
                                                }
                                            }
                                            Text {
                                                text: modelData.created_at ? modelData.created_at : "Path: " + modelData.path
                                                font.pixelSize: 10
                                                color: theme.textMuted
                                            }
                                        }

                                        // Restore button
                                        Rectangle {
                                            Layout.preferredWidth: 80
                                            Layout.preferredHeight: 30
                                            radius: 5
                                            color: rstMouse.containsMouse ? theme.cardHover : theme.sidebarBg
                                            border.color: theme.cardBorder

                                            RowLayout {
                                                anchors.centerIn: parent
                                                spacing: 4
                                                Text { text: fa.history; font.family: faFont.name; font.pixelSize: 10; color: theme.running }
                                                Text { text: "Restore"; font.pixelSize: 11; color: theme.textMain }
                                            }

                                            MouseArea {
                                                id: rstMouse
                                                anchors.fill: parent
                                                hoverEnabled: true
                                                cursorShape: Qt.PointingHandCursor
                                                onClicked: {
                                                    runD4c(["snap", "restore", snapSvcSelector.currentText, "-s", modelData.tag], () => {});
                                                }
                                            }
                                        }

                                        // Export button
                                        Rectangle {
                                            Layout.preferredWidth: 80
                                            Layout.preferredHeight: 30
                                            radius: 5
                                            color: expMouse.containsMouse ? theme.cardHover : theme.sidebarBg
                                            border.color: theme.cardBorder

                                            RowLayout {
                                                anchors.centerIn: parent
                                                spacing: 4
                                                Text { text: fa.archive; font.family: faFont.name; font.pixelSize: 10; color: theme.primaryHover }
                                                Text { text: "Export"; font.pixelSize: 11; color: theme.textMain }
                                            }

                                            MouseArea {
                                                id: expMouse
                                                anchors.fill: parent
                                                hoverEnabled: true
                                                cursorShape: Qt.PointingHandCursor
                                                onClicked: {
                                                    runD4c(["snap", "export", snapSvcSelector.currentText], () => {});
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // ==================== VIEW 3: NETWORKS ====================
                        ColumnLayout {
                            visible: root.activeTab === "networks"
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            spacing: 12

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10

                                TextField {
                                    id: newNetInput
                                    placeholderText: "New bridge network name (e.g. backend-grid)"
                                    Layout.fillWidth: true
                                }
                                Rectangle {
                                    Layout.preferredWidth: 120
                                    Layout.preferredHeight: 36
                                    radius: 6
                                    color: crNetMouse.containsMouse ? theme.primaryHover : theme.primary

                                    RowLayout {
                                        anchors.centerIn: parent
                                        spacing: 6
                                        Text { text: fa.plus; font.family: faFont.name; font.pixelSize: 11; color: theme.primaryText }
                                        Text { text: "Create Net"; font.bold: true; font.pixelSize: 12; color: theme.primaryText }
                                    }

                                    MouseArea {
                                        id: crNetMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            if (newNetInput.text.trim()) {
                                                runD4c(["net", "create", newNetInput.text.trim()], () => {
                                                    newNetInput.text = "";
                                                });
                                            }
                                        }
                                    }
                                }
                            }

                            ListView {
                                id: netListView
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                clip: true
                                spacing: 8
                                model: root.networksList
                                boundsBehavior: Flickable.StopAtBounds

                                ScrollBar.vertical: ScrollBar {
                                    active: true
                                    policy: ScrollBar.AsNeeded
                                }

                                delegate: Rectangle {
                                    width: netListView.width
                                    height: 60
                                    radius: 8
                                    color: theme.cardBg
                                    border.color: theme.cardBorder

                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.margins: 12
                                        spacing: 14

                                        Text {
                                            text: fa.network
                                            font.family: faFont.name
                                            font.pixelSize: 15
                                            color: theme.primaryHover
                                        }

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 2
                                            Text {
                                                text: modelData.name
                                                font.bold: true
                                                font.pixelSize: 13
                                                color: theme.textMain
                                            }
                                            Text {
                                                text: "Subnet: " + (modelData.subnet ? modelData.subnet : "-") + " | Gateway: " + (modelData.gateway ? modelData.gateway : "-") + " | Attached: " + modelData.containers_count
                                                font.pixelSize: 11
                                                color: theme.textSub
                                            }
                                        }

                                        Rectangle {
                                            Layout.preferredWidth: 32
                                            Layout.preferredHeight: 32
                                            radius: 6
                                            color: rmNetMouse.containsMouse ? theme.cardHover : theme.sidebarBg

                                            Text {
                                                anchors.centerIn: parent
                                                text: fa.trash
                                                font.family: faFont.name
                                                font.pixelSize: 12
                                                color: theme.stopped
                                            }

                                            MouseArea {
                                                id: rmNetMouse
                                                anchors.fill: parent
                                                hoverEnabled: true
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

                        // ==================== VIEW 4: LIVE LOGS ====================
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
                                    color: theme.textSub
                                    font.pixelSize: 13
                                }

                                ComboBox {
                                    id: logSelector
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
                                    color: clrLogsMouse.containsMouse ? theme.cardHover : theme.sidebarBg
                                    border.color: theme.cardBorder

                                    Text {
                                        anchors.centerIn: parent
                                        text: "Clear"
                                        font.pixelSize: 11
                                        color: theme.textSub
                                    }

                                    MouseArea {
                                        id: clrLogsMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: root.logContent = ""
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                radius: 8
                                color: "#0a0b10"
                                border.color: theme.cardBorder

                                ScrollView {
                                    anchors.fill: parent
                                    anchors.margins: 10
                                    clip: true

                                    TextArea {
                                        text: root.logContent
                                        readOnly: true
                                        font.family: "monospace"
                                        font.pixelSize: 11
                                        color: theme.textMain
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
                                    text: "d4c Orchestrator | QuickShell Engine"
                                    font.pixelSize: 10
                                    color: theme.textMuted
                                }
                            }
                        }
                    }
                }
            }

            // ==================== IN-WINDOW OVERLAY MODALS ====================
            Rectangle {
                id: modalOverlay
                anchors.fill: parent
                z: 1000
                visible: root.activeModal !== ""
                color: "#cc08090e"

                // Click outside modal card to close
                MouseArea {
                    anchors.fill: parent
                    onClicked: root.activeModal = ""
                }

                // Centered Modal Card
                Rectangle {
                    anchors.centerIn: parent
                    width: 460
                    implicitHeight: modalCol.implicitHeight + 40
                    radius: 12
                    color: theme.modalBg
                    border.color: theme.cardBorder
                    border.width: 1

                    // Block clicks from passing through
                    MouseArea {
                        anchors.fill: parent
                        onClicked: {}
                    }

                    ColumnLayout {
                        id: modalCol
                        anchors.fill: parent
                        anchors.margins: 20
                        spacing: 14

                        // Modal Header
                        RowLayout {
                            Layout.fillWidth: true
                            Text {
                                text: {
                                    if (root.activeModal === "provision") return "Provision Service";
                                    if (root.activeModal === "branch") return "Branch Service Volume";
                                    if (root.activeModal === "snapshot") return "Create Volume Snapshot";
                                    if (root.activeModal === "merge") return "Merge Branch into Target";
                                    if (root.activeModal === "updateLimits") return "Set Resource Limits (CPU & RAM)";
                                    if (root.activeModal === "wipe") return "Wipe Service";
                                    return "";
                                }
                                font.bold: true
                                font.pixelSize: 16
                                color: theme.textMain
                                Layout.fillWidth: true
                            }
                            Text {
                                text: fa.times
                                font.family: faFont.name
                                font.pixelSize: 14
                                color: theme.textSub
                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: root.activeModal = ""
                                }
                            }
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            height: 1
                            color: theme.cardBorder
                        }

                        // ---------- Modal 1: PROVISION ----------
                        ColumnLayout {
                            visible: root.activeModal === "provision"
                            Layout.fillWidth: true
                            spacing: 10

                            Text { text: "Service Catalog:"; color: theme.textSub; font.pixelSize: 12 }
                            ComboBox {
                                id: provCatCombo
                                Layout.fillWidth: true
                                model: ["postgres", "mysql", "mongo", "redis", "kafka", "rabbitmq", "custom"]
                            }

                            Text { text: "Service Name (optional):"; color: theme.textSub; font.pixelSize: 12 }
                            TextField { id: provNameInput; Layout.fillWidth: true; placeholderText: "defaults to catalog name" }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    Text { text: "Host Port (optional):"; color: theme.textSub; font.pixelSize: 12 }
                                    TextField { id: provPortInput; Layout.fillWidth: true; placeholderText: "auto-assign" }
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    Text { text: "Project Tag (optional):"; color: theme.textSub; font.pixelSize: 12 }
                                    TextField { id: provTagInput; Layout.fillWidth: true; placeholderText: "e.g. dev, prod" }
                                }
                            }

                            // Resource Limits Section
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    Text { text: "CPU Limit (e.g. 1.0, 2.0):"; color: theme.textSub; font.pixelSize: 12 }
                                    TextField { id: provCpuInput; Layout.fillWidth: true; placeholderText: "e.g. 1.5 (optional)" }
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    Text { text: "RAM Limit (e.g. 512m, 2g):"; color: theme.textSub; font.pixelSize: 12 }
                                    TextField { id: provMemInput; Layout.fillWidth: true; placeholderText: "e.g. 1g (optional)" }
                                }
                            }

                            RowLayout {
                                Layout.topMargin: 12
                                Item { Layout.fillWidth: true }
                                Button { text: "Cancel"; onClicked: root.activeModal = "" }
                                Button {
                                    text: "Provision Service"
                                    onClicked: {
                                        var args = ["up", provCatCombo.currentText];
                                        if (provNameInput.text.trim()) args.push("--name", provNameInput.text.trim());
                                        if (provPortInput.text.trim()) args.push("--port", provPortInput.text.trim());
                                        if (provTagInput.text.trim()) args.push("--tag", provTagInput.text.trim());
                                        if (provCpuInput.text.trim()) args.push("--cpus", provCpuInput.text.trim());
                                        if (provMemInput.text.trim()) args.push("--memory", provMemInput.text.trim());
                                        runD4c(args, () => {});
                                        root.activeModal = "";
                                    }
                                }
                            }
                        }

                        // ---------- Modal 2: BRANCH ----------
                        ColumnLayout {
                            visible: root.activeModal === "branch"
                            Layout.fillWidth: true
                            spacing: 10

                            Text {
                                text: "Fork volume and instantiate branch for: " + (root.modalTargetService ? root.modalTargetService.name : "")
                                color: theme.textSub
                                font.pixelSize: 12
                            }

                            Text { text: "Branch Suffix Tag:"; color: theme.textSub; font.pixelSize: 12 }
                            TextField { id: branchSuffixInput; Layout.fillWidth: true; placeholderText: "e.g. feat-auth, dev, staging" }

                            Text { text: "Dedicated Port (optional):"; color: theme.textSub; font.pixelSize: 12 }
                            TextField { id: branchPortInput; Layout.fillWidth: true; placeholderText: "auto-assign if empty" }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    Text { text: "CPU Limit (optional):"; color: theme.textSub; font.pixelSize: 12 }
                                    TextField { id: branchCpuInput; Layout.fillWidth: true; placeholderText: "inherit or override" }
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    Text { text: "RAM Limit (optional):"; color: theme.textSub; font.pixelSize: 12 }
                                    TextField { id: branchMemInput; Layout.fillWidth: true; placeholderText: "inherit or override" }
                                }
                            }

                            RowLayout {
                                Layout.topMargin: 12
                                Item { Layout.fillWidth: true }
                                Button { text: "Cancel"; onClicked: root.activeModal = "" }
                                Button {
                                    text: "Create Branch"
                                    onClicked: {
                                        if (branchSuffixInput.text.trim() && root.modalTargetService) {
                                            var args = ["snap", "branch", root.modalTargetService.name, branchSuffixInput.text.trim()];
                                            if (branchPortInput.text.trim()) args.push("--port", branchPortInput.text.trim());
                                            if (branchCpuInput.text.trim()) args.push("--cpus", branchCpuInput.text.trim());
                                            if (branchMemInput.text.trim()) args.push("--memory", branchMemInput.text.trim());
                                            runD4c(args, () => {});
                                            root.activeModal = "";
                                        }
                                    }
                                }
                            }
                        }

                        // ---------- Modal 3: UPDATE RESOURCE LIMITS ----------
                        ColumnLayout {
                            visible: root.activeModal === "updateLimits"
                            Layout.fillWidth: true
                            spacing: 12

                            Text {
                                text: "Update CPU & RAM quota live for: " + (root.modalTargetService ? root.modalTargetService.name : "")
                                color: theme.textSub
                                font.pixelSize: 12
                            }

                            Text { text: "Max CPU limit (e.g. '1.0', '2.5'):"; color: theme.textSub; font.pixelSize: 12 }
                            TextField {
                                id: limitCpuInput
                                Layout.fillWidth: true
                                placeholderText: "e.g. 2.0"
                            }

                            Text { text: "Max Memory limit (e.g. '512m', '2g', '4g'):"; color: theme.textSub; font.pixelSize: 12 }
                            TextField {
                                id: limitMemInput
                                Layout.fillWidth: true
                                placeholderText: "e.g. 2g"
                            }

                            RowLayout {
                                Layout.topMargin: 12
                                Item { Layout.fillWidth: true }
                                Button { text: "Cancel"; onClicked: root.activeModal = "" }
                                Button {
                                    text: "Apply Limits"
                                    onClicked: {
                                        if (root.modalTargetService) {
                                            var args = ["update", root.modalTargetService.name];
                                            if (limitCpuInput.text.trim()) args.push("--cpus", limitCpuInput.text.trim());
                                            if (limitMemInput.text.trim()) args.push("--memory", limitMemInput.text.trim());
                                            runD4c(args, () => {});
                                            root.activeModal = "";
                                        }
                                    }
                                }
                            }
                        }

                        // ---------- Modal 4: SNAPSHOT ----------
                        ColumnLayout {
                            visible: root.activeModal === "snapshot"
                            Layout.fillWidth: true
                            spacing: 10

                            Text {
                                text: "Service: " + (root.modalTargetService ? root.modalTargetService.name : "")
                                color: theme.textSub
                                font.pixelSize: 12
                            }

                            Text { text: "Snapshot Tag (optional):"; color: theme.textSub; font.pixelSize: 12 }
                            TextField { id: snapTagInput; Layout.fillWidth: true; placeholderText: "timestamp if blank" }

                            CheckBox {
                                id: snapRoCheck
                                text: "Read-only snapshot"
                                checked: true
                            }

                            RowLayout {
                                Layout.topMargin: 12
                                Item { Layout.fillWidth: true }
                                Button { text: "Cancel"; onClicked: root.activeModal = "" }
                                Button {
                                    text: "Create Snapshot"
                                    onClicked: {
                                        if (root.modalTargetService) {
                                            var args = ["snap", "create", root.modalTargetService.name];
                                            if (snapTagInput.text.trim()) args.push("--tag", snapTagInput.text.trim());
                                            if (snapRoCheck.checked) args.push("--readonly");
                                            runD4c(args, () => {});
                                            root.activeModal = "";
                                        }
                                    }
                                }
                            }
                        }

                        // ---------- Modal 5: MERGE ----------
                        ColumnLayout {
                            visible: root.activeModal === "merge"
                            Layout.fillWidth: true
                            spacing: 10

                            Text { text: "Source Branch Service:"; color: theme.textSub; font.pixelSize: 12 }
                            ComboBox {
                                id: mergeSrcCombo
                                Layout.fillWidth: true
                                model: root.servicesList.map(s => s.name)
                            }

                            Text { text: "Target Service to receive merged state:"; color: theme.textSub; font.pixelSize: 12 }
                            ComboBox {
                                id: mergeTgtCombo
                                Layout.fillWidth: true
                                model: root.servicesList.map(s => s.name)
                            }

                            Text {
                                text: "Safety notice: An atomic backup snapshot will automatically be taken before merging."
                                color: theme.warning
                                font.pixelSize: 11
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                            }

                            RowLayout {
                                Layout.topMargin: 12
                                Item { Layout.fillWidth: true }
                                Button { text: "Cancel"; onClicked: root.activeModal = "" }
                                Button {
                                    text: "Execute Merge"
                                    onClicked: {
                                        if (mergeSrcCombo.currentText && mergeTgtCombo.currentText && mergeSrcCombo.currentText !== mergeTgtCombo.currentText) {
                                            runD4c(["snap", "merge", mergeSrcCombo.currentText, mergeTgtCombo.currentText], () => {});
                                            root.activeModal = "";
                                        }
                                    }
                                }
                            }
                        }

                        // ---------- Modal 6: WIPE ----------
                        ColumnLayout {
                            visible: root.activeModal === "wipe"
                            Layout.fillWidth: true
                            spacing: 12

                            Text {
                                text: "Are you sure you want to permanently WIPE service '" + (root.modalTargetService ? root.modalTargetService.name : "") + "' and destroy all its Btrfs volume subvolumes?"
                                color: theme.stopped
                                font.bold: true
                                font.pixelSize: 12
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                            }

                            RowLayout {
                                Layout.topMargin: 12
                                Item { Layout.fillWidth: true }
                                Button { text: "Cancel"; onClicked: root.activeModal = "" }
                                Button {
                                    text: "Destroy & Wipe"
                                    onClicked: {
                                        if (root.modalTargetService) {
                                            runD4c(["wipe", root.modalTargetService.name, "--force"], () => {});
                                            root.activeModal = "";
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
