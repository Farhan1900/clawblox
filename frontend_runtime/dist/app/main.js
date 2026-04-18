const projectRootEl = document.getElementById('project-root')
const treeEl = document.getElementById('tree')
const viewerEl = document.getElementById('viewer')
const viewerEmptyEl = document.getElementById('viewer-empty')
const selectionTitleEl = document.getElementById('selection-title')
const selectionMetaEl = document.getElementById('selection-meta')

let selectedPath = null

const ICONS = {
  chevronRight: `
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <path fill="currentColor" d="M6 3.2 10.8 8 6 12.8z"/>
    </svg>
  `,
  folder: `
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <path fill="currentColor" d="M1.75 3.5h4.1l1.2 1.5h7.2v7.75H1.75z"/>
    </svg>
  `,
  file: `
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <path fill="currentColor" d="M3 2.25h6.2L13 6.05v7.7H3z"/>
      <path fill="#1e1e1e" d="M9.2 2.25v3.8H13z"/>
    </svg>
  `,
}

async function fetchJson(path, options = undefined) {
  const resp = await fetch(path, options)
  if (!resp.ok) throw new Error(`${path} HTTP ${resp.status}`)
  return resp.json()
}

function collapseNode(node) {
  if (!node || node.kind !== 'directory' || !Array.isArray(node.children)) return node
  let current = {
    ...node,
    children: node.children.map(collapseNode),
  }
  const names = [current.name]
  while (current.kind === 'directory' && current.children.length === 1 && current.children[0].kind === 'directory') {
    current = current.children[0]
    names.push(current.name)
  }
  return {
    ...current,
    name: names.join('/'),
    children: Array.isArray(current.children) ? current.children.map(collapseNode) : [],
  }
}

function normalizeTree(nodes) {
  return Array.isArray(nodes) ? nodes.map(collapseNode) : []
}

function makeIndent(depth) {
  return `${10 + depth * 14}px`
}

function renderTree(nodes, parent, depth = 0) {
  for (const node of nodes) {
    if (node.kind === 'directory') {
      const group = document.createElement('div')
      group.className = 'tree__group'

      const row = document.createElement('button')
      row.type = 'button'
      row.className = 'tree__row tree__row--dir'
      row.style.paddingLeft = makeIndent(depth)
      row.setAttribute('aria-expanded', 'false')
      row.innerHTML = `
        <span class="tree__caret">${ICONS.chevronRight}</span>
        <span class="tree__icon">${ICONS.folder}</span>
        <span class="tree__label"></span>
      `
      row.querySelector('.tree__label').textContent = node.name

      const children = document.createElement('div')
      children.className = 'tree__children'
      children.hidden = true
      renderTree(node.children || [], children, depth + 1)

      let open = false
      row.querySelector('.tree__caret').style.transform = 'rotate(0deg)'
      row.addEventListener('click', () => {
        open = !open
        children.hidden = !open
        row.setAttribute('aria-expanded', open ? 'true' : 'false')
        row.querySelector('.tree__caret').style.transform = open ? 'rotate(90deg)' : 'rotate(0deg)'
      })

      group.append(row, children)
      parent.appendChild(group)
      continue
    }

    const row = document.createElement('button')
    row.type = 'button'
    row.className = 'tree__row'
    row.dataset.path = node.path
    row.style.paddingLeft = makeIndent(depth)
    row.innerHTML = `
      <span class="tree__spacer"></span>
      <span class="tree__icon">${ICONS.file}</span>
      <span class="tree__label"></span>
    `
    row.querySelector('.tree__label').textContent = node.name
    row.addEventListener('click', () => {
      void openReplay(node.path)
    })
    parent.appendChild(row)
  }
}

function updateSelectedUi(path) {
  selectedPath = path
  for (const el of treeEl.querySelectorAll('.tree__row[data-path]')) {
    el.classList.toggle('is-selected', el.dataset.path === path)
  }
}

function shortMeta(payload) {
  const parts = []
  const world = String(payload.world_dir || '').split('/').filter(Boolean).pop() || ''
  if (world) parts.push(world)
  if (payload.agent_log) parts.push('agent linked')
  return parts.join(' · ')
}

async function openReplay(path) {
  updateSelectedUi(path)
  selectionTitleEl.textContent = path.split('/').pop() || path
  selectionMetaEl.textContent = ''
  viewerEl.src = 'about:blank'
  viewerEl.hidden = true
  viewerEmptyEl.textContent = 'Loading recording...'
  viewerEmptyEl.hidden = false
  try {
    const payload = await fetchJson('api/open-replay', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ path }),
    })
    viewerEl.src = payload.url
    viewerEl.hidden = false
    viewerEmptyEl.hidden = true
    selectionMetaEl.textContent = shortMeta(payload)
  } catch (err) {
    console.error(err)
    viewerEl.src = 'about:blank'
    viewerEl.hidden = true
    viewerEmptyEl.textContent = 'Select a recording'
    viewerEmptyEl.hidden = false
    selectionMetaEl.textContent = ''
    selectionTitleEl.textContent = err instanceof Error ? err.message : 'Failed to open replay'
  }
}

async function bootstrap() {
  try {
    const data = await fetchJson('api/tree')
    projectRootEl.textContent = data.root_name || 'Project'
    treeEl.textContent = ''
    const normalized = normalizeTree(data.tree)
    renderTree(normalized, treeEl)
    if (!treeEl.children.length) {
      viewerEmptyEl.textContent = 'No recordings found'
    }
  } catch (err) {
    console.error(err)
    projectRootEl.textContent = 'Unavailable'
    viewerEmptyEl.textContent = err instanceof Error ? err.message : 'Failed to load tree'
  }
}

bootstrap()
