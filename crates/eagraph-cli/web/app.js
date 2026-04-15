let allData, viewMode = "files", selectedNode = null, currentRepo = null;
const activeKinds = new Set();
const activeLangs = new Set();
const svg = d3.select("svg");
const width = window.innerWidth, height = window.innerHeight;
const g = svg.append("g");
svg.call(d3.zoom().scaleExtent([0.05, 10]).on("zoom", (e) => g.attr("transform", e.transform)));
let linkG = g.append("g").attr("class", "links");
let nodeG = g.append("g").attr("class", "nodes");
let simulation = d3.forceSimulation();

// --- Dropdown logic ---
function populateDropdown(containerId, label, items, defaults, activeSet, onChange) {
  const container = document.querySelector("#" + containerId + " .dropdown-menu");
  container.innerHTML = "";
  activeSet.clear();
  items.forEach(item => {
    const checked = defaults.includes(item);
    if (checked) activeSet.add(item);
    const labelEl = document.createElement("label");
    labelEl.className = "dropdown-item";
    const cb = document.createElement("input");
    cb.type = "checkbox";
    cb.value = item;
    cb.checked = checked;
    cb.addEventListener("change", () => {
      if (cb.checked) activeSet.add(item);
      else activeSet.delete(item);
      updateToggleLabel(containerId, label, activeSet, items.length);
      onChange();
    });
    const span = document.createElement("span");
    span.textContent = item;
    labelEl.appendChild(cb);
    labelEl.appendChild(span);
    container.appendChild(labelEl);
  });
  updateToggleLabel(containerId, label, activeSet, items.length);
}

function updateToggleLabel(containerId, label, activeSet, total) {
  const toggle = document.querySelector("#" + containerId + " .dropdown-toggle");
  if (activeSet.size === total) toggle.textContent = label + ": all";
  else if (activeSet.size === 0) toggle.textContent = label + ": none";
  else toggle.textContent = label + ": " + activeSet.size + "/" + total;
}

// Close dropdowns when clicking outside
document.addEventListener("click", (e) => {
  document.querySelectorAll(".dropdown").forEach(d => {
    if (!d.contains(e.target)) d.classList.remove("open");
  });
});
document.querySelectorAll(".dropdown-toggle").forEach(btn => {
  btn.addEventListener("click", (e) => {
    e.stopPropagation();
    const dd = btn.parentElement;
    const wasOpen = dd.classList.contains("open");
    document.querySelectorAll(".dropdown").forEach(d => d.classList.remove("open"));
    if (!wasOpen) dd.classList.add("open");
  });
});

// --- Repo loading ---
fetch("/repos.json").then(r => r.json()).then(repos => {
  const select = document.getElementById("repo-switcher");
  repos.forEach(name => {
    const opt = document.createElement("option");
    opt.value = name;
    opt.textContent = name;
    select.appendChild(opt);
  });
  select.addEventListener("change", () => loadRepo(select.value));
  if (repos.length > 0) loadRepo(repos[0]);
});

function loadRepo(name) {
  currentRepo = name;
  document.getElementById("repo-switcher").value = name;
  document.title = "eagraph \u2014 " + name;
  fetch("/data/" + encodeURIComponent(name)).then(r => r.json()).then(data => {
    allData = data;
    buildFilters();
    render();
  });
}

function currentData() { return viewMode === "files" ? allData.files : allData.symbols; }

function buildFilters() {
  const src = currentData();
  const kinds = [...new Set(src.nodes.map(n => n.kind))].sort();
  const langs = [...new Set(src.nodes.map(n => n.lang).filter(Boolean))].sort();
  const defaultKinds = viewMode === "files" ? kinds : ["function", "class"];
  populateDropdown("kind-dropdown", "Kind", kinds, defaultKinds, activeKinds, render);
  populateDropdown("lang-dropdown", "Language", langs, langs, activeLangs, render);
}

function getVisible() {
  const src = currentData();
  const nodes = src.nodes.filter(n => activeKinds.has(n.kind) && (!n.lang || activeLangs.has(n.lang)));
  const nodeIds = new Set(nodes.map(n => n.id));
  const links = src.links.filter(l => {
    const s = l.source?.id || l.source, t = l.target?.id || l.target;
    return nodeIds.has(s) && nodeIds.has(t);
  });
  return {
    nodes: nodes.map(n => ({...n})),
    links: links.map(l => ({source: l.source?.id || l.source, target: l.target?.id || l.target, kind: l.kind, weight: l.weight || 1}))
  };
}

function setInfo(name, detail) {
  const el = document.getElementById("info");
  el.innerHTML = "";
  if (!name) { el.innerHTML = "&nbsp;"; return; }
  const nameSpan = document.createElement("span");
  nameSpan.className = "name";
  nameSpan.textContent = name;
  const detailSpan = document.createElement("span");
  detailSpan.className = "detail";
  detailSpan.textContent = detail;
  el.appendChild(nameSpan);
  el.appendChild(detailSpan);
}

function render() {
  selectedNode = null;
  const data = getVisible();
  document.getElementById("stats").textContent = data.nodes.length + " nodes, " + data.links.length + " edges";
  setInfo(null);
  linkG.selectAll("line").remove();
  nodeG.selectAll("g.node").remove();

  const link = linkG.selectAll("line").data(data.links).enter().append("line")
    .attr("class", d => "link " + d.kind)
    .attr("stroke-width", d => viewMode === "files" ? Math.min(Math.sqrt(d.weight), 5) : 1);

  const node = nodeG.selectAll("g.node").data(data.nodes, d => d.id).enter().append("g")
    .attr("class", d => "node " + d.kind);
  const radius = viewMode === "files" ? (d => Math.max(4, Math.sqrt(d.symbols || 1) * 2)) : (() => 5);
  node.append("circle").attr("r", radius);
  node.append("text")
    .attr("dx", d => (viewMode === "files" ? Math.max(6, Math.sqrt(d.symbols || 1) * 2 + 2) : 8))
    .attr("dy", 3).text(d => d.name);

  node.on("click", (e, d) => {
    e.stopPropagation();
    selectedNode = selectedNode === d.id ? null : d.id;
    highlight(data);
    if (selectedNode) {
      const detail = viewMode === "files"
        ? (d.symbols || 0) + " symbols \u2014 " + d.file
        : "(" + d.kind + ") " + d.file + ":" + d.lineStart + "\u2013" + d.lineEnd;
      setInfo(d.name, detail);
    } else setInfo(null);
  });

  node.call(d3.drag()
    .on("start", (e, d) => { if (!e.active) simulation.alphaTarget(0.3).restart(); d.fx = d.x; d.fy = d.y; })
    .on("drag", (e, d) => { d.fx = e.x; d.fy = e.y; })
    .on("end", (e, d) => { if (!e.active) simulation.alphaTarget(0); d.fx = null; d.fy = null; })
  );

  simulation = d3.forceSimulation(data.nodes)
    .force("link", d3.forceLink(data.links).id(d => d.id).distance(viewMode === "files" ? 120 : 80))
    .force("charge", d3.forceManyBody().strength(viewMode === "files" ? -200 : -120))
    .force("center", d3.forceCenter(width / 2, height / 2))
    .force("collision", d3.forceCollide(viewMode === "files" ? 30 : 20))
    .on("tick", () => {
      link.attr("x1", d => d.source.x).attr("y1", d => d.source.y)
          .attr("x2", d => d.target.x).attr("y2", d => d.target.y);
      node.attr("transform", d => "translate(" + d.x + "," + d.y + ")");
    });
}

function highlight(data) {
  if (!selectedNode) {
    nodeG.selectAll("g.node").classed("dimmed", false).classed("highlighted", false);
    linkG.selectAll("line").classed("dimmed", false).classed("highlighted", false);
    return;
  }
  const connected = new Set([selectedNode]);
  data.links.forEach(l => {
    const s = l.source?.id || l.source, t = l.target?.id || l.target;
    if (s === selectedNode) connected.add(t);
    if (t === selectedNode) connected.add(s);
  });
  nodeG.selectAll("g.node").classed("dimmed", d => !connected.has(d.id)).classed("highlighted", d => d.id === selectedNode);
  linkG.selectAll("line")
    .classed("dimmed", d => { const s = d.source?.id||d.source, t = d.target?.id||d.target; return s !== selectedNode && t !== selectedNode; })
    .classed("highlighted", d => { const s = d.source?.id||d.source, t = d.target?.id||d.target; return s === selectedNode || t === selectedNode; });
}

svg.on("click", () => { selectedNode = null; highlight(getVisible()); setInfo(null); });

document.getElementById("search").addEventListener("input", function() {
  const q = this.value.toLowerCase();
  const hasQuery = q.length > 0;
  const matches = new Set();
  nodeG.selectAll("g.node").each(d => {
    if (hasQuery && d.name.toLowerCase().includes(q)) matches.add(d.id);
  });
  nodeG.selectAll("g.node")
    .classed("search-match", d => matches.has(d.id))
    .classed("dimmed", d => hasQuery && !matches.has(d.id));
  linkG.selectAll("line").classed("dimmed", d => {
    if (!hasQuery) return false;
    const s = d.source?.id || d.source, t = d.target?.id || d.target;
    return !matches.has(s) && !matches.has(t);
  });
});

document.querySelectorAll(".view-btn").forEach(btn => {
  btn.addEventListener("click", () => {
    viewMode = btn.dataset.view;
    document.querySelectorAll(".view-btn").forEach(b => b.classList.remove("active"));
    btn.classList.add("active");
    buildFilters();
    render();
  });
});
