function clamp(v, min, max) {
  return Math.max(min, Math.min(max, v))
}

function toColorHex(render, fallback = '#4f46e5') {
  const c = render?.color
  if (!Array.isArray(c) || c.length !== 3) return fallback
  const r = Math.round(clamp(c[0], 0, 1) * 255)
  const g = Math.round(clamp(c[1], 0, 1) * 255)
  const b = Math.round(clamp(c[2], 0, 1) * 255)
  return `rgb(${r}, ${g}, ${b})`
}

function defaultRoughness(material) {
  switch (material) {
    case 'Metal': return 0.22
    case 'Concrete': return 0.72
    case 'Slate': return 0.82
    case 'Glass': return 0.1
    case 'Wood': return 0.85
    case 'Neon': return 0.2
    default: return 0.58
  }
}

function defaultMetalness(material) {
  switch (material) {
    case 'Metal': return 0.92
    case 'Concrete': return 0.22
    case 'Slate': return 0.12
    case 'Glass': return 0.1
    case 'Wood': return 0.06
    case 'Neon': return 0.35
    default: return 0.25
  }
}

export function createPresetMaterialLibrary(initial = {}) {
  const presets = new Map(Object.entries(initial))

  return {
    register(id, preset) {
      if (!id) return
      presets.set(String(id), preset || {})
    },
    get(id) {
      if (!id) return null
      return presets.get(String(id)) || null
    },
    resolve(render) {
      const presetId = render?.preset_id
      if (!presetId) return null
      return presets.get(String(presetId)) || null
    },
    entries() {
      return [...presets.entries()]
    },
  }
}

export function materialFromRender(THREE, render, presetLib = null) {
  const preset = presetLib?.resolve ? presetLib.resolve(render) : null
  const color = new THREE.Color(toColorHex(render, '#8b5cf6'))
  const materialName = render?.material || 'Plastic'
  const emissiveIntensity =
    preset?.emissiveIntensity ?? (materialName === 'Neon' ? 0.9 : 0)

  return new THREE.MeshStandardMaterial({
    color,
    roughness: preset?.roughness ?? defaultRoughness(materialName),
    metalness: preset?.metalness ?? defaultMetalness(materialName),
    emissive: emissiveIntensity > 0 ? color : new THREE.Color(0x000000),
    emissiveIntensity,
    transparent: typeof render?.transparency === 'number' && render.transparency > 0,
    opacity: typeof render?.transparency === 'number' ? Math.max(0, 1 - render.transparency) : 1,
    side: render?.double_sided ? THREE.DoubleSide : THREE.FrontSide,
  })
}

export function geometryFromRender(THREE, render, size = [1, 1, 1]) {
  const sx = Math.max(0.001, size?.[0] ?? 1)
  const sy = Math.max(0.001, size?.[1] ?? 1)
  const sz = Math.max(0.001, size?.[2] ?? 1)
  const primitive = String(render?.primitive || 'box').toLowerCase()

  switch (primitive) {
    case 'ball':
    case 'sphere':
      return new THREE.SphereGeometry(Math.max(0.001, sx * 0.5), 18, 16)
    case 'cylinder': {
      const radius = Math.max(0.001, Math.max(sx, sz) * 0.5)
      return new THREE.CylinderGeometry(radius, radius, sy, 16)
    }
    case 'wedge':
    case 'block':
    case 'box':
    default:
      return new THREE.BoxGeometry(sx, sy, sz)
  }
}

export function applyRendererPreset(THREE, renderer, preset = 'balanced') {
  if (!renderer) return
  renderer.shadowMap.enabled = true
  renderer.shadowMap.type = THREE.PCFSoftShadowMap
  renderer.toneMapping = THREE.ACESFilmicToneMapping
  renderer.outputColorSpace = THREE.SRGBColorSpace

  if (preset === 'cinematic') {
    renderer.toneMappingExposure = 1.35
  } else if (preset === 'performance') {
    renderer.toneMappingExposure = 1.0
    renderer.shadowMap.type = THREE.BasicShadowMap
  } else {
    renderer.toneMappingExposure = 1.15
  }
}

export function applyLightingPreset(THREE, scene, preset = 'arena') {
  const lights = []

  const ambient = new THREE.AmbientLight(0xffffff, preset === 'cinematic' ? 0.35 : 0.6)
  lights.push(ambient)

  const hemi = new THREE.HemisphereLight(0x87ceeb, 0x1c2a3a, preset === 'cinematic' ? 0.25 : 0.4)
  lights.push(hemi)

  const dir = new THREE.DirectionalLight(0xffffff, preset === 'cinematic' ? 1.35 : 1.0)
  dir.position.set(20, 30, 12)
  dir.castShadow = true
  dir.shadow.mapSize.set(2048, 2048)
  dir.shadow.camera.left = -70
  dir.shadow.camera.right = 70
  dir.shadow.camera.top = 70
  dir.shadow.camera.bottom = -70
  dir.shadow.camera.near = 0.5
  dir.shadow.camera.far = 220
  dir.shadow.bias = -0.001
  lights.push(dir)

  for (const l of lights) scene.add(l)
  return lights
}

export function disposeObject3D(root) {
  if (!root) return
  root.traverse((node) => {
    if (node.geometry?.dispose) node.geometry.dispose()
    const mat = node.material
    if (!mat) return
    if (Array.isArray(mat)) {
      for (const m of mat) if (m?.dispose) m.dispose()
    } else if (mat.dispose) {
      mat.dispose()
    }
  })
}

export function createEntityStore(scene, options = {}) {
  const map = new Map()
  const dispose = options.dispose || disposeObject3D

  function upsert(entity, createFn, updateFn) {
    if (!entity) return null
    let object = map.get(entity.id)
    if (!object) {
      object = createFn(entity)
      if (!object) return null
      map.set(entity.id, object)
      if (scene && object.parent !== scene) scene.add(object)
    }
    if (updateFn) updateFn(object, entity)
    return object
  }

  function remove(id) {
    const object = map.get(id)
    if (!object) return
    if (object.parent) object.parent.remove(object)
    dispose(object)
    map.delete(id)
  }

  function prune(activeIds) {
    for (const id of [...map.keys()]) {
      if (!activeIds.has(id)) remove(id)
    }
  }

  function clear() {
    for (const id of [...map.keys()]) remove(id)
  }

  return {
    upsert,
    remove,
    prune,
    clear,
    get(id) {
      return map.get(id) || null
    },
    entries() {
      return [...map.entries()]
    },
    size() {
      return map.size
    },
  }
}

export function createFollowCameraController(THREE, camera, options = {}) {
  const cfg = {
    followDistance: options.followDistance ?? 7.5,
    followHeight: options.followHeight ?? 1.5,
    shoulderOffset: options.shoulderOffset ?? 0.8,
    positionSmoothing: options.positionSmoothing ?? 10,
    rotationSmoothing: options.rotationSmoothing ?? 14,
    minDistance: options.minDistance ?? 3.5,
    lookAheadDistance: options.lookAheadDistance ?? 10,
  }

  const tmpForward = new THREE.Vector3()
  const tmpRight = new THREE.Vector3()
  const desiredPos = new THREE.Vector3()
  const desiredLook = new THREE.Vector3()
  const lookMatrix = new THREE.Matrix4()
  const targetQuat = new THREE.Quaternion()

  function update(targetPosition, targetForward, dt, collisionAdjust = null) {
    if (!targetPosition) return

    const dtSafe = Math.max(1 / 240, Math.min(0.1, dt || 1 / 60))
    const posAlpha = 1 - Math.exp(-cfg.positionSmoothing * dtSafe)
    const rotAlpha = 1 - Math.exp(-cfg.rotationSmoothing * dtSafe)

    tmpForward.copy(targetForward || new THREE.Vector3(0, 0, 1)).setY(0)
    if (tmpForward.lengthSq() < 1e-5) tmpForward.set(0, 0, 1)
    tmpForward.normalize()

    tmpRight.set(-tmpForward.z, 0, tmpForward.x).normalize()

    desiredPos
      .copy(targetPosition)
      .addScaledVector(tmpForward, -Math.max(cfg.minDistance, cfg.followDistance))
      .addScaledVector(tmpRight, cfg.shoulderOffset)
    desiredPos.y += cfg.followHeight

    if (typeof collisionAdjust === 'function') {
      const adjusted = collisionAdjust(desiredPos.clone(), targetPosition.clone())
      if (adjusted) desiredPos.copy(adjusted)
    }

    desiredLook
      .copy(targetPosition)
      .addScaledVector(tmpForward, cfg.lookAheadDistance * 0.25)
    desiredLook.y += cfg.followHeight * 0.5

    camera.position.lerp(desiredPos, posAlpha)
    lookMatrix.lookAt(camera.position, desiredLook, new THREE.Vector3(0, 1, 0))
    targetQuat.setFromRotationMatrix(lookMatrix)
    camera.quaternion.slerp(targetQuat, rotAlpha)
  }

  return {
    update,
    config: cfg,
  }
}

export function createCameraModeController(THREE, camera, options = {}) {
  const followCtl = createFollowCameraController(THREE, camera, options.follow || {})
  const tmpForward = new THREE.Vector3(0, 0, 1)
  const tmpOverviewTarget = new THREE.Vector3()
  const tmpOverviewPos = new THREE.Vector3()
  const spectatorOffset = options.spectatorOffset || new THREE.Vector3(0, 18, -18)
  const defaultSpectatorOffset = spectatorOffset.lengthSq() > 1e-6
    ? spectatorOffset.clone()
    : new THREE.Vector3(0, 18, -18)
  const controlsCanvas = options.canvas || null
  const debugCamera = options.debugCamera ?? true
  const controlsState = {
    mode: 'follow',
    modeOverride: null,
    attached: false,
    disposed: false,
    pan: new THREE.Vector3(0, 0, 0),
    zoom: 1,
    seeded: false,
    keys: new Set(),
    touchPoints: new Map(),
    mouseDrag: null,
    lastTouchCenter: null,
    pinchStartDistance: null,
    pinchStartZoom: null,
    orbitYaw: 0,
    orbitPitch: 0,
    orbitSeeded: false,
    debugTickCount: 0,
  }

  const overviewCfg = {
    panSpeed: options.overviewPanSpeed ?? 55,
    zoomSpeed: options.overviewZoomSpeed ?? 0.001,
    minZoom: options.minOverviewZoom ?? 0.1,
    maxZoom: options.maxOverviewZoom ?? 8,
    touchPanSpeed: options.touchPanSpeed ?? 0.08,
    panDragSign: options.invertOverviewPanDrag === false ? 1 : -1,
    boundsPadding: options.spectatorBoundsPadding ?? -5,
    rotateSpeed: options.overviewRotateSpeed ?? 0.005,
    minPitch: THREE.MathUtils.degToRad(options.minOverviewPitchDeg ?? 12),
    maxPitch: THREE.MathUtils.degToRad(options.maxOverviewPitchDeg ?? 85),
  }

  function dbg(...args) {
    if (!debugCamera) return
    console.log('[runtime-three camera]', ...args)
  }

  function shouldIgnoreKeyboardEvent(target) {
    if (!(target instanceof HTMLElement)) return false
    const tagName = target.tagName.toLowerCase()
    return target.isContentEditable || tagName === 'input' || tagName === 'textarea' || tagName === 'select'
  }

  function distanceBetweenTouches() {
    const points = Array.from(controlsState.touchPoints.values())
    if (points.length < 2) return null
    const dx = points[0].x - points[1].x
    const dy = points[0].y - points[1].y
    return Math.hypot(dx, dy)
  }

  function centerOfTouches() {
    const points = Array.from(controlsState.touchPoints.values())
    if (points.length === 0) return null
    const sum = points.reduce((acc, p) => ({ x: acc.x + p.x, y: acc.y + p.y }), { x: 0, y: 0 })
    return { x: sum.x / points.length, y: sum.y / points.length }
  }

  function computeBoundsFromState(state, padding = 0) {
    let minX = Infinity
    let maxX = -Infinity
    let minZ = Infinity
    let maxZ = -Infinity
    const pushPos = (pos) => {
      if (!Array.isArray(pos)) return
      const x = Number(pos[0])
      const z = Number(pos[2])
      if (!Number.isFinite(x) || !Number.isFinite(z)) return
      minX = Math.min(minX, x)
      maxX = Math.max(maxX, x)
      minZ = Math.min(minZ, z)
      maxZ = Math.max(maxZ, z)
    }
    for (const e of state?.entities || []) pushPos(e?.position)
    for (const p of state?.players || []) pushPos(p?.position)
    if (!Number.isFinite(minX) || !Number.isFinite(maxX) || !Number.isFinite(minZ) || !Number.isFinite(maxZ)) {
      return null
    }
    return {
      minX: minX - padding,
      maxX: maxX + padding,
      minZ: minZ - padding,
      maxZ: maxZ + padding,
    }
  }

  function getSpectatorBounds() {
    if (typeof options.spectatorBounds === 'function') {
      return options.spectatorBounds()
    }
    if (options.spectatorBounds && typeof options.spectatorBounds === 'object') {
      return options.spectatorBounds
    }
    const stateBuffer = options.stateBuffer
    if (stateBuffer && typeof stateBuffer.latest === 'function') {
      return computeBoundsFromState(stateBuffer.latest(), overviewCfg.boundsPadding)
    }
    return null
  }

  function getModeOverride() {
    return controlsState.modeOverride
  }

  function toPlainVec3(v) {
    return { x: Number(v?.x) || 0, y: Number(v?.y) || 0, z: Number(v?.z) || 0 }
  }

  function toPlainQuat(q) {
    return { x: Number(q?.x) || 0, y: Number(q?.y) || 0, z: Number(q?.z) || 0, w: Number(q?.w) || 1 }
  }

  function normalizeMode(mode) {
    if (mode == null || mode === '' || mode === 'default') return null
    const value = String(mode)
    if (value === 'follow' || value === 'spectator' || value === 'first_person') return value
    throw new Error(`Unsupported camera mode: ${mode}`)
  }

  function getCameraPose() {
    return {
      position: toPlainVec3(camera.position),
      quaternion: toPlainQuat(camera.quaternion),
    }
  }

  function getState() {
    const bounds = getSpectatorBounds()
    return {
      mode: controlsState.mode,
      modeOverride: getModeOverride(),
      camera: getCameraPose(),
      spectator: {
        pan: toPlainVec3(controlsState.pan),
        zoom: controlsState.zoom,
        orbitYaw: controlsState.orbitYaw,
        orbitPitch: controlsState.orbitPitch,
        seeded: controlsState.seeded,
        orbitSeeded: controlsState.orbitSeeded,
        bounds: bounds ? { ...bounds } : null,
      },
    }
  }

  function setModeOverride(mode = null) {
    controlsState.modeOverride = normalizeMode(mode)
    return getState()
  }

  function setSpectatorPose(next = {}) {
    if (!next || typeof next !== 'object') return getState()
    const pan = next.pan
    if (pan && Number.isFinite(pan.x) && Number.isFinite(pan.z)) {
      controlsState.pan.set(
        Number(pan.x),
        Number.isFinite(pan.y) ? Number(pan.y) : controlsState.pan.y,
        Number(pan.z),
      )
      controlsState.seeded = true
    }
    if (Number.isFinite(next.zoom)) {
      controlsState.zoom = THREE.MathUtils.clamp(Number(next.zoom), overviewCfg.minZoom, overviewCfg.maxZoom)
    }
    if (Number.isFinite(next.orbitYaw)) {
      controlsState.orbitYaw = Number(next.orbitYaw)
      controlsState.orbitSeeded = true
    }
    if (Number.isFinite(next.orbitPitch)) {
      controlsState.orbitPitch = THREE.MathUtils.clamp(
        Number(next.orbitPitch),
        overviewCfg.minPitch,
        overviewCfg.maxPitch
      )
      controlsState.orbitSeeded = true
    }
    if (typeof next.seeded === 'boolean') controlsState.seeded = next.seeded
    clampPanToBounds()
    return getState()
  }

  function resetSpectator(targetPosition = null) {
    if (targetPosition && Number.isFinite(targetPosition.x) && Number.isFinite(targetPosition.z)) {
      controlsState.pan.set(
        Number(targetPosition.x),
        Number.isFinite(targetPosition.y) ? Number(targetPosition.y) : controlsState.pan.y,
        Number(targetPosition.z)
      )
    }
    controlsState.zoom = 1
    controlsState.orbitSeeded = false
    seedOrbitFromOffset()
    controlsState.seeded = !!targetPosition
    clampPanToBounds()
    return getState()
  }

  function clampPanToBounds() {
    const bounds = getSpectatorBounds()
    if (!bounds) return
    const beforeX = controlsState.pan.x
    const beforeZ = controlsState.pan.z
    controlsState.pan.x = THREE.MathUtils.clamp(controlsState.pan.x, bounds.minX, bounds.maxX)
    controlsState.pan.z = THREE.MathUtils.clamp(controlsState.pan.z, bounds.minZ, bounds.maxZ)
    if (debugCamera && (beforeX !== controlsState.pan.x || beforeZ !== controlsState.pan.z)) {
      dbg('clampPanToBounds', { beforeX, beforeZ, afterX: controlsState.pan.x, afterZ: controlsState.pan.z, bounds })
    }
  }

  function panByScreenDelta(dx, dy) {
    seedOrbitFromOffset()
    const scale = overviewCfg.touchPanSpeed * controlsState.zoom
    const before = { x: controlsState.pan.x, z: controlsState.pan.z }
    const cosYaw = Math.cos(controlsState.orbitYaw)
    const sinYaw = Math.sin(controlsState.orbitYaw)
    const worldDx = (dx * cosYaw) + (dy * sinYaw)
    const worldDz = (-dx * sinYaw) + (dy * cosYaw)
    controlsState.pan.x += worldDx * scale * overviewCfg.panDragSign
    controlsState.pan.z += worldDz * scale * overviewCfg.panDragSign
    clampPanToBounds()
    dbg('panByScreenDelta', {
      dx,
      dy,
      worldDx,
      worldDz,
      scale,
      panDragSign: overviewCfg.panDragSign,
      yaw: controlsState.orbitYaw,
      before,
      after: { x: controlsState.pan.x, z: controlsState.pan.z },
    })
  }

  function seedOrbitFromOffset() {
    if (controlsState.orbitSeeded) return
    const horizontal = Math.hypot(defaultSpectatorOffset.x, defaultSpectatorOffset.z)
    controlsState.orbitYaw = Math.atan2(defaultSpectatorOffset.x, defaultSpectatorOffset.z)
    controlsState.orbitPitch = Math.atan2(defaultSpectatorOffset.y, Math.max(1e-6, horizontal))
    controlsState.orbitPitch = THREE.MathUtils.clamp(
      controlsState.orbitPitch,
      overviewCfg.minPitch,
      overviewCfg.maxPitch
    )
    controlsState.orbitSeeded = true
    dbg('seedOrbit', {
      yaw: controlsState.orbitYaw,
      pitch: controlsState.orbitPitch,
    })
  }

  function rotateByScreenDelta(dx, dy) {
    seedOrbitFromOffset()
    const before = { yaw: controlsState.orbitYaw, pitch: controlsState.orbitPitch }
    controlsState.orbitYaw -= dx * overviewCfg.rotateSpeed
    controlsState.orbitPitch = THREE.MathUtils.clamp(
      controlsState.orbitPitch + dy * overviewCfg.rotateSpeed,
      overviewCfg.minPitch,
      overviewCfg.maxPitch
    )
    dbg('rotateByScreenDelta', {
      dx,
      dy,
      before,
      after: { yaw: controlsState.orbitYaw, pitch: controlsState.orbitPitch },
    })
  }

  function resetTouchState() {
    if (controlsState.touchPoints.size < 2) {
      controlsState.pinchStartDistance = null
      controlsState.pinchStartZoom = null
    }
    controlsState.lastTouchCenter = centerOfTouches()
  }

  function endMouseDrag() {
    controlsState.mouseDrag = null
  }

  function attachSpectatorControls() {
    if (!controlsCanvas || controlsState.attached) return
    controlsState.attached = true
    controlsCanvas.style.touchAction = 'none'
    dbg('attachSpectatorControls', {
      hasCanvas: !!controlsCanvas,
      touchAction: controlsCanvas.style.touchAction,
      overviewCfg,
      spectatorOffset: { x: spectatorOffset.x, y: spectatorOffset.y, z: spectatorOffset.z },
    })

    const onKeyDown = (event) => {
      if (controlsState.mode !== 'spectator') return
      if (shouldIgnoreKeyboardEvent(event.target)) return
      const key = event.key.toLowerCase()
      if (key === 'w' || key === 'a' || key === 's' || key === 'd') {
        controlsState.keys.add(key)
        dbg('keydown', key, 'held=', [...controlsState.keys])
      }
    }

    const onKeyUp = (event) => {
      const key = event.key.toLowerCase()
      controlsState.keys.delete(key)
      if (key === 'w' || key === 'a' || key === 's' || key === 'd') {
        dbg('keyup', key, 'held=', [...controlsState.keys])
      }
    }

    const onWindowBlur = () => {
      controlsState.keys.clear()
      dbg('windowBlur')
    }

    const onWheel = (event) => {
      if (controlsState.mode !== 'spectator') return
      event.preventDefault()
      const before = controlsState.zoom
      const nextZoom = controlsState.zoom * Math.exp(event.deltaY * overviewCfg.zoomSpeed)
      controlsState.zoom = THREE.MathUtils.clamp(nextZoom, overviewCfg.minZoom, overviewCfg.maxZoom)
      dbg('wheel', { deltaY: event.deltaY, before, after: controlsState.zoom })
    }

    const onContextMenu = (event) => {
      if (controlsState.mode !== 'spectator') return
      event.preventDefault()
    }

    const onPointerDown = (event) => {
      if (controlsState.mode !== 'spectator') return
      if (event.pointerType === 'mouse') {
        const wantsRotate = event.button === 2 || (event.button === 0 && event.shiftKey)
        const wantsPan = event.button === 0 && !event.shiftKey
        if (!wantsRotate && !wantsPan) return
        controlsState.mouseDrag = {
          id: event.pointerId,
          x: event.clientX,
          y: event.clientY,
          mode: wantsRotate ? 'rotate' : 'pan',
        }
        try { controlsCanvas.setPointerCapture?.(event.pointerId) } catch {}
        dbg('mouseDownStart', {
          id: event.pointerId,
          x: event.clientX,
          y: event.clientY,
          mode: controlsState.mouseDrag.mode,
          shiftKey: !!event.shiftKey,
          button: event.button,
        })
        event.preventDefault()
        return
      }
      if (event.pointerType !== 'touch') return
      controlsState.touchPoints.set(event.pointerId, { x: event.clientX, y: event.clientY })
      controlsState.lastTouchCenter = centerOfTouches()
      dbg('pointerdown', { id: event.pointerId, type: event.pointerType, x: event.clientX, y: event.clientY, count: controlsState.touchPoints.size })
      if (controlsState.touchPoints.size >= 2) {
        controlsState.pinchStartDistance = distanceBetweenTouches()
        controlsState.pinchStartZoom = controlsState.zoom
        dbg('pinchStart', { dist: controlsState.pinchStartDistance, zoom: controlsState.pinchStartZoom })
      }
    }

    const onPointerMove = (event) => {
      if (controlsState.mode !== 'spectator') return
      if (event.pointerType === 'mouse') {
        const md = controlsState.mouseDrag
        if (!md || md.id !== event.pointerId) return
        const dx = event.clientX - md.x
        const dy = event.clientY - md.y
        md.x = event.clientX
        md.y = event.clientY
        if (md.mode === 'rotate') {
          rotateByScreenDelta(dx, dy)
          dbg('mouseDragRotate', { id: event.pointerId, dx, dy })
        } else {
          panByScreenDelta(dx, dy)
          dbg('mouseDragPan', { id: event.pointerId, dx, dy })
        }
        event.preventDefault()
        return
      }
      if (event.pointerType !== 'touch') return
      const previous = controlsState.touchPoints.get(event.pointerId)
      if (!previous) return
      event.preventDefault()
      controlsState.touchPoints.set(event.pointerId, { x: event.clientX, y: event.clientY })
      dbg('pointermove', { id: event.pointerId, dx: event.clientX - previous.x, dy: event.clientY - previous.y, count: controlsState.touchPoints.size })

      if (controlsState.touchPoints.size >= 2) {
        const center = centerOfTouches()
        const dist = distanceBetweenTouches()
        if (center && controlsState.lastTouchCenter) {
          panByScreenDelta(center.x - controlsState.lastTouchCenter.x, center.y - controlsState.lastTouchCenter.y)
        }
        if (dist && controlsState.pinchStartDistance && controlsState.pinchStartZoom) {
          const ratio = dist / controlsState.pinchStartDistance
          const nextZoom = controlsState.pinchStartZoom / ratio
          controlsState.zoom = THREE.MathUtils.clamp(nextZoom, overviewCfg.minZoom, overviewCfg.maxZoom)
          dbg('pinchZoom', { dist, ratio, zoom: controlsState.zoom })
        }
        controlsState.lastTouchCenter = center
      } else {
        panByScreenDelta(event.clientX - previous.x, event.clientY - previous.y)
      }
    }

    const onPointerEnd = (event) => {
      if (event.pointerType === 'mouse') {
        if (controlsState.mouseDrag?.id === event.pointerId) {
          dbg('mouseDownEnd', { id: event.pointerId, mode: controlsState.mouseDrag.mode })
          endMouseDrag()
          try { controlsCanvas.releasePointerCapture?.(event.pointerId) } catch {}
        }
        return
      }
      if (event.pointerType !== 'touch') return
      controlsState.touchPoints.delete(event.pointerId)
      resetTouchState()
      dbg('pointerend', { id: event.pointerId, count: controlsState.touchPoints.size })
    }

    controlsCanvas.addEventListener('wheel', onWheel, { passive: false })
    controlsCanvas.addEventListener('pointerdown', onPointerDown)
    controlsCanvas.addEventListener('pointermove', onPointerMove, { passive: false })
    controlsCanvas.addEventListener('pointerup', onPointerEnd)
    controlsCanvas.addEventListener('pointercancel', onPointerEnd)
    controlsCanvas.addEventListener('contextmenu', onContextMenu)
    window.addEventListener('keydown', onKeyDown)
    window.addEventListener('keyup', onKeyUp)
    window.addEventListener('blur', onWindowBlur)

    controlsState.dispose = () => {
      dbg('disposeSpectatorControls')
      controlsCanvas.removeEventListener('wheel', onWheel)
      controlsCanvas.removeEventListener('pointerdown', onPointerDown)
      controlsCanvas.removeEventListener('pointermove', onPointerMove)
      controlsCanvas.removeEventListener('pointerup', onPointerEnd)
      controlsCanvas.removeEventListener('pointercancel', onPointerEnd)
      controlsCanvas.removeEventListener('contextmenu', onContextMenu)
      window.removeEventListener('keydown', onKeyDown)
      window.removeEventListener('keyup', onKeyUp)
      window.removeEventListener('blur', onWindowBlur)
      controlsState.attached = false
      controlsState.keys.clear()
      controlsState.touchPoints.clear()
      endMouseDrag()
      controlsState.lastTouchCenter = null
      controlsState.pinchStartDistance = null
      controlsState.pinchStartZoom = null
    }
  }

  function update({ mode = 'follow', targetPosition, targetForward, dt = 1 / 60, collisionAdjust = null }) {
    if (!targetPosition) return
    const effectiveMode = controlsState.modeOverride || mode
    controlsState.mode = effectiveMode
    if (debugCamera) {
      controlsState.debugTickCount += 1
      if (controlsState.debugTickCount <= 10 || controlsState.debugTickCount % 60 === 0) {
        dbg('update', {
          mode: effectiveMode,
          dt,
          targetPosition: { x: targetPosition.x, y: targetPosition.y, z: targetPosition.z },
          pan: { x: controlsState.pan.x, y: controlsState.pan.y, z: controlsState.pan.z },
          zoom: controlsState.zoom,
          keys: [...controlsState.keys],
          attached: controlsState.attached,
        })
      }
    }

    if (effectiveMode === 'spectator' && controlsCanvas) {
      attachSpectatorControls()
    }

    if (effectiveMode === 'first_person') {
      const pos = targetPosition.clone()
      pos.y += options.firstPersonHeight ?? 1.6
      camera.position.copy(pos)

      const lookTarget = pos.clone().add(targetForward || tmpForward)
      camera.lookAt(lookTarget)
      return
    }

    if (effectiveMode === 'spectator') {
      const dtSafe = Math.max(1 / 240, Math.min(0.1, dt || 1 / 60))
      if (!controlsState.seeded) {
        controlsState.pan.set(targetPosition.x, targetPosition.y, targetPosition.z)
        controlsState.seeded = true
        dbg('seedSpectatorPan', { x: controlsState.pan.x, y: controlsState.pan.y, z: controlsState.pan.z })
      }

      if (controlsCanvas) {
        const direction = new THREE.Vector3()
        if (controlsState.keys.has('w')) direction.z -= 1
        if (controlsState.keys.has('s')) direction.z += 1
        if (controlsState.keys.has('a')) direction.x -= 1
        if (controlsState.keys.has('d')) direction.x += 1

        if (direction.lengthSq() > 0) {
          direction.normalize()
          const step = (overviewCfg.panSpeed * controlsState.zoom) * dtSafe
          seedOrbitFromOffset()
          const cosYaw = Math.cos(controlsState.orbitYaw)
          const sinYaw = Math.sin(controlsState.orbitYaw)
          const localDx = -direction.x * step
          const localDz = -direction.z * step
          const worldDx = (localDx * cosYaw) + (localDz * sinYaw)
          const worldDz = (-localDx * sinYaw) + (localDz * cosYaw)
          const before = { x: controlsState.pan.x, z: controlsState.pan.z }
          controlsState.pan.x += worldDx
          controlsState.pan.z += worldDz
          clampPanToBounds()
          dbg('keyboardPan', {
            step,
            yaw: controlsState.orbitYaw,
            localDx,
            localDz,
            worldDx,
            worldDz,
            before,
            after: { x: controlsState.pan.x, z: controlsState.pan.z },
            keys: [...controlsState.keys],
          })
        }

        seedOrbitFromOffset()
        const baseDistance = Math.max(0.001, defaultSpectatorOffset.length())
        const orbitDistance = baseDistance * controlsState.zoom
        const cosPitch = Math.cos(controlsState.orbitPitch)
        const sinPitch = Math.sin(controlsState.orbitPitch)
        const sinYaw = Math.sin(controlsState.orbitYaw)
        const cosYaw = Math.cos(controlsState.orbitYaw)
        tmpOverviewTarget.set(controlsState.pan.x, controlsState.pan.y, controlsState.pan.z)
        tmpOverviewPos.set(
          controlsState.pan.x + sinYaw * cosPitch * orbitDistance,
          controlsState.pan.y + sinPitch * orbitDistance,
          controlsState.pan.z + cosYaw * cosPitch * orbitDistance
        )
        camera.position.copy(tmpOverviewPos)
        camera.lookAt(tmpOverviewTarget)
        if (debugCamera && (controlsState.debugTickCount <= 10 || controlsState.debugTickCount % 60 === 0)) {
          dbg('spectatorCameraPose', {
            position: { x: camera.position.x, y: camera.position.y, z: camera.position.z },
            lookAt: { x: tmpOverviewTarget.x, y: tmpOverviewTarget.y, z: tmpOverviewTarget.z },
          })
        }
        return
      }

      tmpOverviewPos.set(
        controlsState.pan.x + spectatorOffset.x,
        controlsState.pan.y + spectatorOffset.y,
        controlsState.pan.z + spectatorOffset.z
      )
      tmpOverviewTarget.set(controlsState.pan.x, controlsState.pan.y, controlsState.pan.z)
      camera.position.lerp(tmpOverviewPos, 1 - Math.exp(-6 * dtSafe))
      camera.lookAt(tmpOverviewTarget)
      return
    }

    followCtl.update(targetPosition, targetForward, dt, collisionAdjust)
  }

  return {
    update,
    follow: followCtl,
    api: {
      getState,
      getCameraPose,
      setMode(mode) {
        return setModeOverride(mode)
      },
      clearModeOverride() {
        return setModeOverride(null)
      },
      setSpectatorPose,
      resetSpectator,
    },
    dispose() {
      if (typeof controlsState.dispose === 'function') controlsState.dispose()
      controlsState.disposed = true
    },
    spectator: {
      state: controlsState,
      reset(targetPosition = null) {
        return resetSpectator(targetPosition)
      },
    },
  }
}

export function buildEntityMesh(THREE, entity, presetLib = null) {
  const render = entity?.render || {}
  const size = entity?.size || [1, 1, 1]
  const mesh = new THREE.Mesh(
    geometryFromRender(THREE, render, size),
    materialFromRender(THREE, render, presetLib),
  )

  mesh.castShadow = render?.casts_shadow !== false
  mesh.receiveShadow = render?.receives_shadow !== false
  mesh.visible = render?.visible !== false
  return mesh
}

export function applyEntityTransform(THREE, object3d, entity) {
  if (!object3d || !entity) return
  const p = entity.position || [0, 0, 0]
  object3d.position.set(p[0] || 0, p[1] || 0, p[2] || 0)

  if (entity.rotation && Array.isArray(entity.rotation) && entity.rotation.length === 3) {
    const r = entity.rotation
    const m = new THREE.Matrix4()
    m.set(
      r[0][0], r[0][1], r[0][2], 0,
      r[1][0], r[1][1], r[1][2], 0,
      r[2][0], r[2][1], r[2][2], 0,
      0, 0, 0, 1,
    )
    object3d.quaternion.setFromRotationMatrix(m)
  }
}

export function createModelTemplateCache() {
  const templates = new Map()

  return {
    async get(url, loadFn) {
      let pending = templates.get(url)
      if (!pending) {
        pending = Promise.resolve(loadFn(url))
        templates.set(url, pending)
      }
      return pending
    },
    clear() {
      templates.clear()
    },
    size() {
      return templates.size
    },
  }
}

function resolveEntityModelUrl(entity) {
  if (typeof entity?.model_url === 'string' && entity.model_url.length > 0) {
    return entity.model_url
  }
  const attrs = entity?.attributes
  if (attrs && typeof attrs.ModelUrl === 'string' && attrs.ModelUrl.length > 0) {
    return attrs.ModelUrl
  }
  return null
}

function fitModelToSize(THREE, model, size = [1, 1, 1]) {
  let mesh = null
  model.traverse((obj) => {
    if (mesh) return
    if (obj?.isSkinnedMesh || obj?.isMesh) mesh = obj
  })

  const sourceBox = new THREE.Box3()
  if (mesh?.geometry) {
    mesh.geometry.computeBoundingBox()
    const bb = mesh.geometry.boundingBox
    if (!bb) return
    sourceBox.copy(bb)
  } else {
    sourceBox.setFromObject(model)
  }

  const src = sourceBox.getSize(new THREE.Vector3())
  if (src.y <= 0.0001) return
  const center = sourceBox.getCenter(new THREE.Vector3())
  const scale = (size?.[1] ?? 1) / src.y
  model.scale.setScalar(scale)
  model.position.set(-center.x * scale, -center.y * scale, -center.z * scale)
}

function entityQuaternionFromRotation(THREE, rotation) {
  const q = new THREE.Quaternion()
  if (!rotation || !Array.isArray(rotation) || rotation.length !== 3) return q
  const m = new THREE.Matrix4()
  m.set(
    rotation[0][0], rotation[0][1], rotation[0][2], 0,
    rotation[1][0], rotation[1][1], rotation[1][2], 0,
    rotation[2][0], rotation[2][1], rotation[2][2], 0,
    0, 0, 0, 1,
  )
  return q.setFromRotationMatrix(m)
}

function entityMatrixFromState(THREE, entity) {
  const pos = entity?.position || [0, 0, 0]
  const p = new THREE.Vector3(pos[0] || 0, pos[1] || 0, pos[2] || 0)
  return new THREE.Matrix4().compose(
    p,
    entityQuaternionFromRotation(THREE, entity?.rotation),
    new THREE.Vector3(1, 1, 1),
  )
}

function writeEntityMatrixFromState(THREE, entity, target, position, quaternion, scale, rotationMatrix) {
  const pos = entity?.position || [0, 0, 0]
  position.set(pos[0] || 0, pos[1] || 0, pos[2] || 0)
  scale.set(1, 1, 1)
  if (entity?.rotation && Array.isArray(entity.rotation) && entity.rotation.length === 3) {
    const rot = entity.rotation
    rotationMatrix.set(
      rot[0][0], rot[0][1], rot[0][2], 0,
      rot[1][0], rot[1][1], rot[1][2], 0,
      rot[2][0], rot[2][1], rot[2][2], 0,
      0, 0, 0, 1,
    )
    quaternion.setFromRotationMatrix(rotationMatrix)
  } else {
    quaternion.identity()
  }
  target.compose(position, quaternion, scale)
  return target
}

function nextCapacity(required) {
  let capacity = 1
  while (capacity < required) capacity *= 2
  return capacity
}

export function createSharedModelRenderer(THREE, scene, options = {}) {
  const minInstances = Math.max(2, Number(options.minInstances ?? 20))
  const templateCache = createModelTemplateCache()
  const groups = new Map()
  const moduleCache = { loader: null, pending: null }
  const tmpSourceBox = new THREE.Box3()
  const tmpSize = new THREE.Vector3()
  const tmpCenter = new THREE.Vector3()
  const tmpEntityMatrix = new THREE.Matrix4()
  const tmpYawMatrix = new THREE.Matrix4()
  const tmpFitMatrix = new THREE.Matrix4()
  const tmpMeshMatrix = new THREE.Matrix4()
  const tmpMatrix = new THREE.Matrix4()
  const tmpEntityPosition = new THREE.Vector3()
  const tmpEntityQuaternion = new THREE.Quaternion()
  const tmpEntityScale = new THREE.Vector3(1, 1, 1)
  const tmpEntityRotationMatrix = new THREE.Matrix4()

  async function ensureModelModules() {
    if (moduleCache.loader) return moduleCache
    if (!moduleCache.pending) {
      moduleCache.pending = import('https://esm.sh/three@0.160.0/examples/jsm/loaders/GLTFLoader.js')
        .then((loaderMod) => {
          moduleCache.loader = new loaderMod.GLTFLoader()
          return moduleCache
        })
    }
    return moduleCache.pending
  }

  async function loadTemplate(modelUrl) {
    const mods = await ensureModelModules()
    return templateCache.get(modelUrl, () => new Promise((resolve, reject) => {
      mods.loader.load(
        modelUrl,
        (gltf) => resolve({ scene: gltf.scene, animations: gltf.animations || [] }),
        undefined,
        reject,
      )
    }))
  }

  function isEligibleTemplate(template) {
    if (!template?.scene || template.animations?.length > 0) return false
    let eligible = true
    template.scene.traverse((node) => {
      if (node?.isSkinnedMesh || Array.isArray(node?.material)) eligible = false
    })
    return eligible
  }

  function extractStaticMeshes(template) {
    template.scene.updateMatrixWorld(true)
    const meshes = []
    tmpSourceBox.makeEmpty()
    template.scene.traverse((node) => {
      if (!node?.isMesh || node.isSkinnedMesh || !node.geometry || Array.isArray(node.material)) return
      node.geometry.computeBoundingBox()
      meshes.push({
        geometry: node.geometry,
        material: node.material,
        matrixWorld: node.matrixWorld.clone(),
      })
      tmpSourceBox.expandByObject(node)
    })
    if (meshes.length === 0 || tmpSourceBox.isEmpty()) return null
    tmpSourceBox.getSize(tmpSize)
    tmpSourceBox.getCenter(tmpCenter)
    if (tmpSize.y <= 0.0001) return null
    return {
      meshes,
      sourceHeight: tmpSize.y,
      sourceCenter: tmpCenter.clone(),
    }
  }

  function getGroup(modelUrl) {
    let group = groups.get(modelUrl)
    if (!group) {
      group = {
        modelUrl,
        entities: new Map(),
        frameIds: new Set(),
        dirtyIds: new Set(),
        indexById: new Map(),
        structureDirty: true,
        lastCount: 0,
        status: 'pending',
        template: null,
        staticTemplate: null,
        instancedMeshes: [],
        fallback: null,
      }
      groups.set(modelUrl, group)
      void loadTemplate(modelUrl)
        .then((template) => {
          group.template = template
          if (isEligibleTemplate(template)) {
            group.staticTemplate = extractStaticMeshes(template)
          }
          group.status = group.staticTemplate ? 'ready' : 'unsupported'
          if (group.status === 'unsupported' && options.onUnsupported) {
            options.onUnsupported({ modelUrl, reason: 'skinned-animated-or-complex' })
          }
        })
        .catch((err) => {
          group.status = 'error'
          if (options.onError) options.onError(err, { modelUrl, phase: 'load-shared-model' })
        })
    }
    return group
  }

  function removeInstancedMeshes(group) {
    for (const mesh of group.instancedMeshes) {
      scene.remove(mesh)
      mesh.material?.dispose?.()
    }
    group.instancedMeshes = []
  }

  function ensureInstancedMeshes(group, count) {
    if (!group.staticTemplate) return
    const capacity = nextCapacity(count)
    if (
      group.instancedMeshes.length === group.staticTemplate.meshes.length &&
      group.instancedMeshes.every((mesh) => mesh.userData.capacity >= count)
    ) {
      return
    }

    removeInstancedMeshes(group)
    for (const source of group.staticTemplate.meshes) {
      const material = source.material.clone()
      if (options.prepareMaterial) options.prepareMaterial(material, { modelUrl: group.modelUrl, source })
      const mesh = new THREE.InstancedMesh(source.geometry, material, capacity)
      mesh.castShadow = true
      mesh.receiveShadow = true
      mesh.frustumCulled = false
      mesh.userData.capacity = capacity
      scene.add(mesh)
      group.instancedMeshes.push(mesh)
    }
  }

  function ensureFallback(group, count) {
    if (group.status === 'ready') {
      if (group.fallback) {
        scene.remove(group.fallback)
        group.fallback.geometry?.dispose?.()
        group.fallback.material?.dispose?.()
        group.fallback = null
      }
      return
    }
    const capacity = nextCapacity(count)
    if (group.fallback && group.fallback.userData.capacity >= count) return
    if (group.fallback) {
      scene.remove(group.fallback)
      group.fallback.geometry?.dispose?.()
      group.fallback.material?.dispose?.()
    }
    const fallback = new THREE.InstancedMesh(
      new THREE.BoxGeometry(1, 1, 1),
      new THREE.MeshStandardMaterial({ color: 0x888888, roughness: 0.8, metalness: 0.05, vertexColors: true }),
      capacity,
    )
    fallback.castShadow = true
    fallback.receiveShadow = true
    fallback.frustumCulled = false
    fallback.userData.capacity = capacity
    group.fallback = fallback
    scene.add(fallback)
  }

  function writeFallbackMatrix(group, entity, index) {
    if (!group.fallback) return
    const size = entity.size || [1, 1, 1]
    writeEntityMatrixFromState(
      THREE,
      entity,
      tmpEntityMatrix,
      tmpEntityPosition,
      tmpEntityQuaternion,
      tmpEntityScale,
      tmpEntityRotationMatrix,
    )
    tmpFitMatrix.makeScale(size[0] || 1, size[1] || 1, size[2] || 1)
    tmpMatrix.multiplyMatrices(tmpEntityMatrix, tmpFitMatrix)
    group.fallback.setMatrixAt(index, tmpMatrix)
  }

  function updateFallback(group, entities) {
    if (!group.fallback) return
    group.fallback.count = entities.length
    entities.forEach((entity, index) => {
      writeFallbackMatrix(group, entity, index)
    })
    group.fallback.instanceMatrix.needsUpdate = true
  }

  function updateFallbackPartial(group) {
    if (!group.fallback || group.dirtyIds.size === 0) return
    for (const id of group.dirtyIds) {
      const entity = group.entities.get(id)
      const index = group.indexById.get(id)
      if (!entity || index == null) continue
      writeFallbackMatrix(group, entity, index)
    }
    group.fallback.instanceMatrix.needsUpdate = true
  }

  function writeInstancedMatrix(group, source, sourceCenter, sourceHeight, entity, index, instancedMesh) {
    const size = entity.size || [1, 1, 1]
    const scale = (size[1] ?? 1) / sourceHeight
    writeEntityMatrixFromState(
      THREE,
      entity,
      tmpEntityMatrix,
      tmpEntityPosition,
      tmpEntityQuaternion,
      tmpEntityScale,
      tmpEntityRotationMatrix,
    )
    tmpYawMatrix.identity()
    if (entity.model_yaw_offset_deg) {
      tmpYawMatrix.makeRotationY(THREE.MathUtils.degToRad(entity.model_yaw_offset_deg))
    }
    tmpFitMatrix.makeScale(scale, scale, scale)
    tmpFitMatrix.setPosition(-sourceCenter.x * scale, -sourceCenter.y * scale, -sourceCenter.z * scale)
    tmpMeshMatrix.copy(source.matrixWorld)
    tmpMatrix.multiplyMatrices(tmpFitMatrix, tmpMeshMatrix)
    tmpMatrix.premultiply(tmpYawMatrix)
    tmpMatrix.premultiply(tmpEntityMatrix)
    instancedMesh.setMatrixAt(index, tmpMatrix)
  }

  function updateInstanced(group, entities) {
    if (!group.staticTemplate) return
    if (group.fallback) {
      scene.remove(group.fallback)
      group.fallback.geometry?.dispose?.()
      group.fallback.material?.dispose?.()
      group.fallback = null
    }
    ensureInstancedMeshes(group, entities.length)
    const sourceCenter = group.staticTemplate.sourceCenter
    const sourceHeight = group.staticTemplate.sourceHeight

    for (let meshIndex = 0; meshIndex < group.staticTemplate.meshes.length; meshIndex += 1) {
      const instancedMesh = group.instancedMeshes[meshIndex]
      const source = group.staticTemplate.meshes[meshIndex]
      instancedMesh.count = entities.length
      entities.forEach((entity, index) => {
        writeInstancedMatrix(group, source, sourceCenter, sourceHeight, entity, index, instancedMesh)
      })
      instancedMesh.instanceMatrix.needsUpdate = true
    }
  }

  function updateInstancedPartial(group) {
    if (!group.staticTemplate || group.dirtyIds.size === 0) return
    const sourceCenter = group.staticTemplate.sourceCenter
    const sourceHeight = group.staticTemplate.sourceHeight

    for (let meshIndex = 0; meshIndex < group.staticTemplate.meshes.length; meshIndex += 1) {
      const instancedMesh = group.instancedMeshes[meshIndex]
      const source = group.staticTemplate.meshes[meshIndex]
      for (const id of group.dirtyIds) {
        const entity = group.entities.get(id)
        const index = group.indexById.get(id)
        if (!entity || index == null) continue
        writeInstancedMatrix(group, source, sourceCenter, sourceHeight, entity, index, instancedMesh)
      }
      instancedMesh.instanceMatrix.needsUpdate = true
    }
  }

  function modelUrlForEntity(entity) {
    if (options.getModelUrl) return options.getModelUrl(entity)
    return resolveEntityModelUrl(entity)
  }

  let frameCounts = new Map()
  let frameChangedIds = null

  function beginFrame(entities = [], frameOptions = null) {
    frameCounts = new Map()
    frameChangedIds = frameOptions?.changedEntityIds instanceof Set
      ? frameOptions.changedEntityIds
      : Array.isArray(frameOptions?.changedEntityIds)
        ? new Set(frameOptions.changedEntityIds)
        : null
    for (const group of groups.values()) {
      group.frameIds.clear()
      group.dirtyIds.clear()
      group.structureDirty = false
    }
    for (const entity of entities || []) {
      const modelUrl = modelUrlForEntity(entity)
      if (!modelUrl) continue
      if (options.isEligible && !options.isEligible(entity, modelUrl)) continue
      frameCounts.set(modelUrl, (frameCounts.get(modelUrl) || 0) + 1)
    }
  }

  function accepts(entity) {
    const modelUrl = modelUrlForEntity(entity)
    if (!modelUrl) return false
    if (options.isEligible && !options.isEligible(entity, modelUrl)) return false
    if ((frameCounts.get(modelUrl) || 0) < minInstances) return false
    const group = getGroup(modelUrl)
    if (group.status === 'unsupported' || group.status === 'error') return false
    const previous = group.entities.get(entity.id)
    group.entities.set(entity.id, entity)
    group.frameIds.add(entity.id)
    if (!previous) group.structureDirty = true
    if (!previous || previous !== entity || frameChangedIds?.has(entity.id)) {
      group.dirtyIds.add(entity.id)
    }
    return true
  }

  function endFrame() {
    for (const group of groups.values()) {
      for (const id of [...group.entities.keys()]) {
        if (!group.frameIds.has(id)) {
          group.entities.delete(id)
          group.structureDirty = true
        }
      }
      const entities = [...group.entities.values()]
      if (entities.length !== group.lastCount) group.structureDirty = true
      if (entities.length === 0) {
        removeInstancedMeshes(group)
        if (group.fallback) {
          scene.remove(group.fallback)
          group.fallback.geometry?.dispose?.()
          group.fallback.material?.dispose?.()
          group.fallback = null
        }
        group.indexById.clear()
        group.lastCount = 0
        continue
      }
      if (group.structureDirty || group.indexById.size !== entities.length) {
        group.indexById.clear()
        entities.forEach((entity, index) => group.indexById.set(entity.id, index))
      }
      if (group.status === 'ready') {
        const needsFullInstancedUpdate =
          group.structureDirty ||
          group.instancedMeshes.length !== (group.staticTemplate?.meshes.length || 0)
        if (needsFullInstancedUpdate) updateInstanced(group, entities)
        else updateInstancedPartial(group)
      } else {
        ensureFallback(group, entities.length)
        if (group.structureDirty) updateFallback(group, entities)
        else updateFallbackPartial(group)
      }
      group.lastCount = entities.length
    }
  }

  function remove(entityId) {
    for (const group of groups.values()) {
      if (group.entities.delete(entityId)) {
        group.indexById.delete(entityId)
        group.structureDirty = true
      }
    }
  }

  function clear() {
    for (const group of groups.values()) {
      removeInstancedMeshes(group)
      if (group.fallback) {
        scene.remove(group.fallback)
        group.fallback.geometry?.dispose?.()
        group.fallback.material?.dispose?.()
      }
    }
    groups.clear()
    templateCache.clear()
  }

  return {
    beginFrame,
    accepts,
    endFrame,
    remove,
    clear,
  }
}

export function createModelEntityController(THREE, options = {}) {
  const templateCache = createModelTemplateCache()
  const modelStates = new Map()
  const moduleCache = { loader: null, cloneFn: null, pending: null }

  async function ensureModelModules() {
    if (moduleCache.loader && moduleCache.cloneFn) return moduleCache
    if (!moduleCache.pending) {
      moduleCache.pending = Promise.all([
        import('https://esm.sh/three@0.160.0/examples/jsm/loaders/GLTFLoader.js'),
        import('https://esm.sh/three@0.160.0/examples/jsm/utils/SkeletonUtils.js'),
      ]).then(([loaderMod, skeletonMod]) => {
        moduleCache.loader = new loaderMod.GLTFLoader()
        moduleCache.cloneFn = skeletonMod.clone
        return moduleCache
      })
    }
    return moduleCache.pending
  }

  async function loadTemplate(modelUrl) {
    const mods = await ensureModelModules()
    return templateCache.get(modelUrl, () => new Promise((resolve, reject) => {
      mods.loader.load(
        modelUrl,
        (gltf) => resolve({ scene: gltf.scene, animations: gltf.animations || [] }),
        undefined,
        reject,
      )
    }))
  }

  if (options.preloadModules !== false) {
    void ensureModelModules().catch((err) => {
      if (options.onError) options.onError(err, { phase: 'preload-modules' })
    })
  }

  function chooseClip(animations, pattern, fallbackIndex = -1) {
    const found = animations.find((c) => pattern.test(c.name))
    if (found) return found
    if (fallbackIndex >= 0 && animations.length > fallbackIndex) return animations[fallbackIndex]
    return null
  }

  async function upsert({ entityId, root, modelUrl, size, yawOffsetDeg }) {
    if (!entityId || !root || !modelUrl) return

    let state = modelStates.get(entityId)
    if (state?.modelUrl === modelUrl && state.ready) return

    if (!state) {
      state = {
        modelUrl,
        ready: false,
        loading: false,
        mixer: null,
        walkAction: null,
        idleAction: null,
      }
      modelStates.set(entityId, state)
    } else {
      state.modelUrl = modelUrl
    }

    if (state.loading) return
    state.loading = true

    try {
      const template = await loadTemplate(modelUrl)
      const mods = await ensureModelModules()
      const current = modelStates.get(entityId)
      if (!current || current.modelUrl !== modelUrl) return

      for (const child of [...root.children]) root.remove(child)

      const clone = mods.cloneFn(template.scene)
      fitModelToSize(THREE, clone, size)
      if (yawOffsetDeg) clone.rotation.y = THREE.MathUtils.degToRad(yawOffsetDeg)
      clone.traverse((node) => {
        if (node?.isMesh) {
          node.castShadow = true
          node.receiveShadow = true
        }
      })
      root.add(clone)

      current.mixer = null
      current.walkAction = null
      current.idleAction = null
      if (template.animations.length > 0) {
        const mixer = new THREE.AnimationMixer(clone)
        const walkClip = chooseClip(template.animations, /(walk|run|jog|locomotion)/i, 0)
        const idleClip = chooseClip(template.animations, /idle/i, -1)
        const walkAction = walkClip ? mixer.clipAction(walkClip) : null
        const idleAction = idleClip ? mixer.clipAction(idleClip) : null

        if (idleAction) {
          idleAction.reset()
          idleAction.play()
        }
        if (walkAction) {
          walkAction.reset()
          walkAction.play()
          walkAction.setEffectiveWeight(0)
        }

        current.mixer = mixer
        current.walkAction = walkAction
        current.idleAction = idleAction
      }
      current.ready = true
    } catch (err) {
      if (options.onError) options.onError(err, { entityId, modelUrl })
    } finally {
      const cur = modelStates.get(entityId)
      if (cur) cur.loading = false
    }
  }

  function update(entityId, { dt = 1 / 60, speed = 0, humanoidState = null } = {}) {
    const state = modelStates.get(entityId)
    if (!state?.mixer) return

    const airborne = humanoidState === 'Jumping' || humanoidState === 'Freefall'
    const moving = speed > 0.5 && !airborne
    if (state.walkAction) {
      const targetWeight = moving ? 1 : 0
      const current = state.walkAction.getEffectiveWeight()
      state.walkAction.setEffectiveWeight(current + (targetWeight - current) * Math.min(1, dt * 8))
      if (moving) {
        state.walkAction.paused = false
        state.walkAction.setEffectiveTimeScale(Math.min(speed / 6, 2))
      }
    }
    if (state.idleAction) {
      const targetWeight = moving ? 0 : 1
      const current = state.idleAction.getEffectiveWeight()
      state.idleAction.setEffectiveWeight(current + (targetWeight - current) * Math.min(1, dt * 8))
    }
    state.mixer.update(dt)
  }

  function remove(entityId) {
    modelStates.delete(entityId)
  }

  function clear() {
    modelStates.clear()
    templateCache.clear()
  }

  return {
    preload(modelUrl) {
      if (!modelUrl) return
      void loadTemplate(modelUrl).catch((err) => {
        if (options.onError) options.onError(err, { modelUrl, phase: 'preload' })
      })
    },
    upsert,
    update,
    remove,
    clear,
  }
}

export function classifyAnimationTracks(player) {
  const tracks = Array.isArray(player?.active_animations) ? player.active_animations : []

  function best(regex) {
    let chosen = null
    let bestWeight = -1
    for (const track of tracks) {
      if (!track || track.is_playing === false) continue
      const id = String(track.animation_id || '')
      if (!regex.test(id)) continue
      const weight = Number(track.effective_weight ?? track.weight_current ?? 1)
      if (weight > bestWeight) {
        bestWeight = weight
        chosen = track
      }
    }
    return chosen
  }

  return {
    fire: best(/fire/i),
    reload: best(/reload/i),
    walk: best(/(walk|run|jog|locomotion)/i),
    idle: best(/idle/i),
    all: tracks,
  }
}

export const defaults = {
  defaultRoughness,
  defaultMetalness,
  toColorHex,
}
