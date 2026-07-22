const state = {
  nodes: [],
  selectedNodeUuid: null,
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
  pairNode: document.querySelector("#pairNode"),
  screenToggle: document.querySelector("#screenToggle"),
  statusLine: document.querySelector("#statusLine"),
};

function tauriInvoke(command) {
  const invoke = window.__TAURI__?.core?.invoke;
  if (!invoke) {
    return Promise.resolve([]);
  }

  return invoke(command);
}

function selectedNode() {
  return state.nodes.find((node) => node.node_uuid === state.selectedNodeUuid) ?? null;
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
    subtitle.textContent = `${node.host_name}:${node.api_port}`;

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

  elements.detailSubtitle.textContent = hasNode ? node.node_uuid : "No node selected";
  elements.pairingState.textContent = hasNode ? "Ready" : "Unpaired";
  elements.pairingState.classList.toggle("muted", !hasNode);
  elements.nodeName.textContent = node?.node_name ?? "-";
  elements.hostName.textContent = node?.host_name ?? "-";
  elements.apiVersion.textContent = node?.api_version ?? "-";
  elements.fingerprint.textContent = node?.node_certificate_fingerprint ?? "-";
  elements.pairNode.disabled = true;
  elements.screenToggle.disabled = true;
}

function render() {
  renderNodes();
  renderDetail();
}

async function refreshNodes() {
  elements.refreshNodes.disabled = true;
  elements.scanState.textContent = "Scanning";
  elements.statusLine.textContent = "Scanning for SecondWind nodes";

  try {
    const nodes = await tauriInvoke("discover_nodes");
    state.nodes = Array.isArray(nodes) ? nodes : [];

    if (!state.nodes.some((node) => node.node_uuid === state.selectedNodeUuid)) {
      state.selectedNodeUuid = state.nodes[0]?.node_uuid ?? null;
    }

    elements.statusLine.textContent = state.nodes.length === 0 ? "No nodes found" : "Nodes updated";
  } catch (error) {
    elements.statusLine.textContent = String(error);
  } finally {
    elements.scanState.textContent = "Idle";
    elements.refreshNodes.disabled = false;
    render();
  }
}

elements.refreshNodes.addEventListener("click", refreshNodes);
render();
refreshNodes();
