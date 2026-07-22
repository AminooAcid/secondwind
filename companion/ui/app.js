const state = {
  nodes: [],
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

function render() {
  renderNodes();
  renderDetail();
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
