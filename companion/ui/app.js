const state = {
  nodes: [],
  apps: [],
  // Devices of the selected paired node, refreshed on selection.
  usbDevices: [],
  // app_id -> true while a launch is in flight or awaiting a choice
  appPendingChoice: new Set(),
  selectedNodeUuid: null,
  // node_uuid -> { display_name, node_certificate_fingerprint, screen_always_on }
  paired: new Map(),
  // node_uuid -> true while the screen is on
  screenOn: new Map(),
  // node_uuid -> drive letter (or "") while the disk is attached
  diskOn: new Map(),
  busy: false,
};

const elements = {
  refreshNodes: document.querySelector("#refreshNodes"),
  nodeCount: document.querySelector("#nodeCount"),
  scanState: document.querySelector("#scanState"),
  nodeList: document.querySelector("#nodeList"),
  detailSubtitle: document.querySelector("#detailSubtitle"),
  pairingState: document.querySelector("#pairingState"),
  nodeName: document.querySelector("#nodeName"),
  hostName: document.querySelector("#hostName"),
  apiVersion: document.querySelector("#apiVersion"),
  fingerprint: document.querySelector("#fingerprint"),
  pairForm: document.querySelector("#pairForm"),
  pinInput: document.querySelector("#pinInput"),
  pairNode: document.querySelector("#pairNode"),
  screenToggle: document.querySelector("#screenToggle"),
  diskToggle: document.querySelector("#diskToggle"),
  statusLine: document.querySelector("#statusLine"),
  appList: document.querySelector("#appList"),
  usbList: document.querySelector("#usbList"),
};

function tauriInvoke(command, args) {
  const invoke = window.__TAURI__?.core?.invoke;
  if (!invoke) {
    return Promise.reject(new Error("SecondWind is still starting."));
  }

  return invoke(command, args);
}

function selectedNode() {
  return state.nodes.find((node) => node.node_uuid === state.selectedNodeUuid) ?? null;
}

function isPaired(nodeUuid) {
  return state.paired.has(nodeUuid);
}

function setStatus(message) {
  elements.statusLine.textContent = message;
}

function renderNodes() {
  elements.nodeList.innerHTML = "";
  elements.nodeCount.textContent = `${state.nodes.length} found`;

  if (state.nodes.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty-state";
    empty.textContent = "No SecondWind nodes found.";
    elements.nodeList.append(empty);
    return;
  }

  for (const node of state.nodes) {
    const item = document.createElement("button");
    item.type = "button";
    item.className = "node-item";
    item.setAttribute("role", "listitem");
    item.setAttribute("aria-selected", String(node.node_uuid === state.selectedNodeUuid));

    const title = document.createElement("span");
    title.className = "node-title";
    title.textContent = node.node_name;

    const subtitle = document.createElement("span");
    subtitle.className = "node-subtitle";
    subtitle.textContent = isPaired(node.node_uuid)
      ? state.screenOn.get(node.node_uuid)
        ? "Paired · screen on"
        : "Paired"
      : "Not paired yet";

    item.append(title, subtitle);
    item.addEventListener("click", () => {
      state.selectedNodeUuid = node.node_uuid;
      render();
      refreshUsb();
    });

    elements.nodeList.append(item);
  }
}

function renderDetail() {
  const node = selectedNode();
  const hasNode = Boolean(node);
  const paired = hasNode && isPaired(node.node_uuid);
  const screenOn = hasNode && Boolean(state.screenOn.get(node.node_uuid));

  elements.detailSubtitle.textContent = hasNode ? node.node_uuid : "No node selected";
  elements.pairingState.textContent = !hasNode ? "—" : paired ? "Paired" : "Waiting to pair";
  elements.pairingState.classList.toggle("muted", !paired);
  elements.nodeName.textContent = node?.node_name ?? "-";
  elements.hostName.textContent = node?.host_name ?? "-";
  elements.apiVersion.textContent = node?.api_version ?? "-";
  elements.fingerprint.textContent = node?.node_certificate_fingerprint ?? "-";

  const diskOn = hasNode && state.diskOn.has(node.node_uuid);

  elements.pairForm.hidden = !hasNode || paired;
  elements.pairNode.disabled = !hasNode || paired || state.busy;
  elements.screenToggle.disabled = !paired || state.busy;
  elements.screenToggle.textContent = screenOn ? "Turn Screen Off" : "Turn Screen On";
  elements.diskToggle.disabled = !paired || state.busy;
  elements.diskToggle.textContent = diskOn
    ? `Detach Disk${state.diskOn.get(node.node_uuid) ? ` (${state.diskOn.get(node.node_uuid)}:)` : ""}`
    : "Attach Disk";
}

const POLICY_LABELS = {
  always_on_node: "Always on node",
  always_local: "Always local",
  ask_each_time: "Ask each time",
};

function renderApps() {
  elements.appList.innerHTML = "";

  if (state.apps.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty-state";
    empty.textContent = "The app library is loading…";
    elements.appList.append(empty);
    return;
  }

  for (const app of state.apps) {
    const row = document.createElement("div");
    row.className = "app-row";
    row.setAttribute("role", "listitem");

    const name = document.createElement("span");
    name.className = "app-name";
    name.textContent = app.display_name;

    const policy = document.createElement("select");
    for (const [value, label] of Object.entries(POLICY_LABELS)) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = label;
      option.selected = app.policy === value;
      policy.append(option);
    }
    policy.addEventListener("change", () => saveAppPolicy(app, policy.value, fallback.checked));

    const fallbackLabel = document.createElement("label");
    fallbackLabel.className = "fallback";
    const fallback = document.createElement("input");
    fallback.type = "checkbox";
    fallback.checked = app.fallback_to_local;
    fallback.addEventListener("change", () => saveAppPolicy(app, policy.value, fallback.checked));
    fallbackLabel.append(fallback, document.createTextNode("Fall back to this PC"));

    row.append(name, policy, fallbackLabel);

    if (state.appPendingChoice.has(app.app_id)) {
      const choice = document.createElement("span");
      choice.className = "app-choice";
      const onNode = document.createElement("button");
      onNode.type = "button";
      onNode.textContent = "On node";
      onNode.addEventListener("click", () => launchApp(app, true));
      const local = document.createElement("button");
      local.type = "button";
      local.textContent = "On this PC";
      local.addEventListener("click", () => launchApp(app, false));
      choice.append(onNode, local);
      row.append(choice);
    } else {
      const launch = document.createElement("button");
      launch.type = "button";
      launch.textContent = "Launch";
      launch.disabled = state.busy;
      launch.addEventListener("click", () => launchApp(app, null));
      row.append(launch);
    }

    elements.appList.append(row);
  }
}

async function refreshApps() {
  try {
    state.apps = await tauriInvoke("app_library");
  } catch (error) {
    state.apps = [];
  }
  render();
}

async function saveAppPolicy(app, policy, fallbackToLocal) {
  try {
    await tauriInvoke("set_app_policy", {
      appId: app.app_id,
      policy,
      fallbackToLocal,
    });
    app.policy = policy;
    app.fallback_to_local = fallbackToLocal;
    setStatus(`${app.display_name}: ${POLICY_LABELS[policy].toLowerCase()}.`);
  } catch (error) {
    setStatus(String(error));
  }
  render();
}

async function launchApp(app, choiceOnNode) {
  state.appPendingChoice.delete(app.app_id);
  setStatus(`Starting ${app.display_name}…`);
  render();

  try {
    const result = await tauriInvoke("launch_app", {
      appId: app.app_id,
      node: selectedNode(),
      choiceOnNode,
    });

    switch (result.outcome) {
      case "on_node":
        setStatus(`${app.display_name} is running on your node.`);
        break;
      case "local":
        setStatus(`${app.display_name} is running on this PC.`);
        break;
      case "needs_choice":
        state.appPendingChoice.add(app.app_id);
        setStatus(`Where should ${app.display_name} run?`);
        break;
      case "failed":
        setStatus(result.message ?? `${app.display_name} could not be started.`);
        break;
    }
  } catch (error) {
    setStatus(String(error));
  }
  render();
}

function renderUsb() {
  elements.usbList.innerHTML = "";
  const node = selectedNode();
  if (!node || !isPaired(node.node_uuid)) {
    const empty = document.createElement("div");
    empty.className = "node-subtitle";
    empty.textContent = "Pair with the node to see its USB ports.";
    elements.usbList.append(empty);
    return;
  }

  if (state.usbDevices.length === 0) {
    const empty = document.createElement("div");
    empty.className = "node-subtitle";
    empty.textContent = "No USB devices plugged into the node.";
    elements.usbList.append(empty);
    return;
  }

  for (const device of state.usbDevices) {
    const row = document.createElement("div");
    row.className = "usb-row";
    row.setAttribute("role", "listitem");

    const name = document.createElement("span");
    name.className = "usb-name";
    name.textContent = device.description || `${device.vendor_id}:${device.product_id}`;

    const attach = document.createElement("button");
    attach.type = "button";
    attach.textContent = device.bound ? "Detach" : "Attach";
    attach.disabled = state.busy;
    attach.addEventListener("click", () => toggleUsb(device));

    const autoLabel = document.createElement("label");
    autoLabel.className = "auto";
    const auto = document.createElement("input");
    auto.type = "checkbox";
    auto.checked = Boolean(device.auto_attach);
    auto.addEventListener("change", () => setUsbAuto(device, auto.checked));
    autoLabel.append(auto, document.createTextNode("Always attach"));

    row.append(name, attach, autoLabel);
    elements.usbList.append(row);
  }
}

async function refreshUsb() {
  const node = selectedNode();
  if (!node || !isPaired(node.node_uuid)) {
    state.usbDevices = [];
    renderUsb();
    return;
  }

  try {
    const response = await tauriInvoke("usb_devices", { node });
    state.usbDevices = response.devices ?? [];
  } catch (error) {
    state.usbDevices = [];
  }
  renderUsb();
}

async function toggleUsb(device) {
  const node = selectedNode();
  if (!node) {
    return;
  }

  state.busy = true;
  render();
  setStatus(device.bound ? "Detaching the device…" : "Attaching the device…");

  try {
    await tauriInvoke("set_usb_attached", {
      node,
      busId: device.bus_id,
      vendorId: device.vendor_id,
      productId: device.product_id,
      attached: !device.bound,
    });
    setStatus(device.bound ? "Device detached." : "Device attached — check Explorer.");
  } catch (error) {
    setStatus(String(error));
  } finally {
    state.busy = false;
    await refreshUsb();
    render();
  }
}

async function setUsbAuto(device, enabled) {
  const node = selectedNode();
  if (!node) {
    return;
  }

  try {
    await tauriInvoke("set_usb_auto_attach", {
      nodeUuid: node.node_uuid,
      vendorId: device.vendor_id,
      productId: device.product_id,
      enabled,
    });
    device.auto_attach = enabled;
    setStatus(enabled ? "This device will attach automatically." : "Auto-attach turned off.");
  } catch (error) {
    setStatus(String(error));
  }
}

function render() {
  renderNodes();
  renderDetail();
  renderApps();
  renderUsb();
}

async function refreshPaired() {
  try {
    const paired = await tauriInvoke("paired_nodes");
    state.paired = new Map(paired.map((node) => [node.node_uuid, node]));
  } catch (error) {
    // Paired list is best-effort at startup; discovery errors surface below.
  }
}

async function refreshNodes() {
  elements.refreshNodes.disabled = true;
  elements.scanState.textContent = "Scanning";
  setStatus("Scanning for SecondWind nodes");

  try {
    const nodes = await tauriInvoke("discover_nodes");
    state.nodes = Array.isArray(nodes) ? nodes : [];

    if (!state.nodes.some((node) => node.node_uuid === state.selectedNodeUuid)) {
      state.selectedNodeUuid = state.nodes[0]?.node_uuid ?? null;
    }

    setStatus(state.nodes.length === 0 ? "No nodes found" : "Nodes updated");
  } catch (error) {
    setStatus(String(error));
  } finally {
    elements.scanState.textContent = "Idle";
    elements.refreshNodes.disabled = false;
    render();
  }
}

async function pairSelectedNode() {
  const node = selectedNode();
  if (!node || state.busy) {
    return;
  }

  state.busy = true;
  render();
  setStatus(`Pairing with ${node.node_name}…`);

  try {
    const summary = await tauriInvoke("pair_node", {
      node,
      pin: elements.pinInput.value,
    });
    state.paired.set(summary.node_uuid, summary);
    elements.pinInput.value = "";
    setStatus(`Paired with ${summary.display_name}.`);
    refreshUsb();
  } catch (error) {
    setStatus(String(error));
  } finally {
    state.busy = false;
    render();
  }
}

async function toggleScreen() {
  const node = selectedNode();
  if (!node || !isPaired(node.node_uuid) || state.busy) {
    return;
  }

  const turningOn = !state.screenOn.get(node.node_uuid);
  state.busy = true;
  render();
  setStatus(turningOn ? "Turning the screen on…" : "Turning the screen off…");

  try {
    const result = await tauriInvoke("set_screen", { node, enabled: turningOn });
    state.screenOn.set(node.node_uuid, result.streaming);
    if (result.streaming) {
      setStatus(`${node.node_name} is now an extra screen.`);
    } else if (turningOn) {
      setStatus(result.message ?? "The node could not start the screen.");
    } else {
      setStatus("Screen turned off.");
    }
  } catch (error) {
    setStatus(String(error));
  } finally {
    state.busy = false;
    render();
  }
}

function listenForAutoConnect() {
  const listen = window.__TAURI__?.event?.listen;
  if (!listen) {
    return;
  }

  listen("secondwind://node-connected", (event) => {
    const payload = event.payload ?? {};
    if (payload.node_uuid) {
      state.screenOn.set(payload.node_uuid, true);
    }
    setStatus(payload.message ?? "Node connected.");
    render();
  });

  listen("secondwind://node-disconnected", (event) => {
    const payload = event.payload ?? {};
    if (payload.node_uuid) {
      state.screenOn.set(payload.node_uuid, false);
    }
    setStatus(payload.message ?? "Node disconnected.");
    render();
  });
}

async function toggleDisk() {
  const node = selectedNode();
  if (!node || !isPaired(node.node_uuid) || state.busy) {
    return;
  }

  const attaching = !state.diskOn.has(node.node_uuid);
  state.busy = true;
  render();
  setStatus(attaching ? "Attaching the node disk…" : "Detaching the node disk…");

  try {
    const result = await tauriInvoke("set_disk", { node, enabled: attaching });
    if (result.attached) {
      state.diskOn.set(node.node_uuid, result.drive_letter ?? "");
      setStatus(
        result.drive_letter
          ? `Node disk attached as ${result.drive_letter}:.`
          : "Node disk attached.",
      );
    } else {
      state.diskOn.delete(node.node_uuid);
      setStatus(attaching ? (result.message ?? "Could not attach the disk.") : "Node disk detached.");
    }
  } catch (error) {
    setStatus(String(error));
  } finally {
    state.busy = false;
    render();
  }
}

elements.refreshNodes.addEventListener("click", refreshNodes);
elements.pairNode.addEventListener("click", pairSelectedNode);
elements.pinInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    pairSelectedNode();
  }
});
elements.screenToggle.addEventListener("click", toggleScreen);
elements.diskToggle.addEventListener("click", toggleDisk);

listenForAutoConnect();
render();
refreshPaired().then(refreshNodes);
refreshApps();
