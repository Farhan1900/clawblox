import * as THREE from 'https://esm.sh/three@0.160.0'

function rotationToQuaternion(rot) {
  const m = new THREE.Matrix4()
  m.set(
    rot[0][0], rot[0][1], rot[0][2], 0,
    rot[1][0], rot[1][1], rot[1][2], 0,
    rot[2][0], rot[2][1], rot[2][2], 0,
    0, 0, 0, 1,
  )
  return new THREE.Quaternion().setFromRotationMatrix(m)
}

function resolveEntityModelUrl(entity) {
  if (typeof entity.model_url === 'string' && entity.model_url.length > 0) {
    return entity.model_url
  }
  const attrs = entity.attributes
  if (attrs && typeof attrs.ModelUrl === 'string' && attrs.ModelUrl.length > 0) {
    return attrs.ModelUrl
  }
  return null
}

function resolveEntityColorHex(entity) {
  const color = entity?.render?.color
  if (!Array.isArray(color) || color.length < 3) {
    console.warn(`Entity ${entity?.id ?? '?'} (role=${entity?.render?.role ?? '?'}) missing render.color`)
    return 0xff00ff
  }
  return (Math.round(color[0] * 255) << 16) |
    (Math.round(color[1] * 255) << 8) |
    Math.round(color[2] * 255)
}

function resolveEntityColor(entity) {
  const color = entity?.render?.color
  if (!Array.isArray(color) || color.length < 3) {
    console.warn(`Entity ${entity?.id ?? '?'} (role=${entity?.render?.role ?? '?'}) missing render.color`)
    return new THREE.Color(0xff00ff)
  }
  return new THREE.Color(color[0], color[1], color[2])
}

function darkenHex(hex, factor = 0.75) {
  const color = new THREE.Color(hex)
  color.multiplyScalar(factor)
  return color.getHex()
}

function createRockLikeMaterial(color) {
  return new THREE.MeshStandardMaterial({
    color,
    roughness: 0.95,
    metalness: 0.0,
    flatShading: true,
  })
}

function applyEntityAppearance(obj, entity) {
  if (!obj?.userData) return

  if (obj.userData.dynamicRole === 'Skuttler') {
    const bodyHex = resolveEntityColorHex(entity)
    const legHex = darkenHex(bodyHex, 0.72)
    if (obj.userData.skuttlerBodyMaterial) {
      obj.userData.skuttlerBodyMaterial.color.setHex(bodyHex)
    }
    if (obj.userData.skuttlerLegMaterial) {
      obj.userData.skuttlerLegMaterial.color.setHex(legHex)
    }
  }

  if (obj.userData.dynamicRole === 'GlowBulb') {
    const size = entity.size || [0.7, 0.7, 0.7]
    obj.scale.set(
      (size[0] || 0.7) / 0.7,
      (size[1] || 0.7) / 0.7,
      (size[2] || 0.7) / 0.7,
    )
  }

  if ((obj.userData.dynamicRole === 'StoneBlock' || obj.userData.dynamicRole === 'StickyBlock') && obj.material) {
    obj.material.color.setHex(resolveEntityColorHex(entity))
  }

  if ((obj.userData.dynamicRole === 'StickyClump' || obj.userData.dynamicRole === 'Clump') && obj.material) {
    obj.material.color.setHex(resolveEntityColorHex(entity))
  }
}

function buildSky(scene) {
  const skyMat = new THREE.ShaderMaterial({
    uniforms: {
      topColor: { value: new THREE.Color(0x87ceeb) },
      bottomColor: { value: new THREE.Color(0xffecd2) },
    },
    vertexShader: `
      varying vec3 vWP;
      void main(){
        vWP = (modelMatrix * vec4(position, 1.0)).xyz;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: `
      uniform vec3 topColor, bottomColor;
      varying vec3 vWP;
      void main(){
        float h = normalize(vWP).y;
        gl_FragColor = vec4(mix(bottomColor, topColor, max(h, 0.0)), 1.0);
      }
    `,
    side: THREE.BackSide,
    depthWrite: false,
  })
  scene.add(new THREE.Mesh(new THREE.SphereGeometry(1500, 32, 32), skyMat))
}

function buildClouds(scene) {
  const cloudList = []
  const cloudMat = new THREE.MeshStandardMaterial({
    color: 0xffffff,
    transparent: true,
    opacity: 0.8,
    roughness: 1,
  })

  for (let i = 0; i < 20; i++) {
    const cg = new THREE.Group()
    for (let j = 0; j < 3 + Math.floor(Math.random() * 4); j++) {
      const p = new THREE.Mesh(new THREE.SphereGeometry(6 + Math.random() * 10, 8, 8), cloudMat)
      p.position.set(j * 8 - 16, Math.random() * 5, Math.random() * 6 - 3)
      p.scale.y = 0.6
      cg.add(p)
    }
    cg.position.set(Math.random() * 500 - 250, 50 + Math.random() * 50, Math.random() * 300 - 150)
    scene.add(cg)
    cloudList.push({ mesh: cg, speed: 0.5 + Math.random() * 1.5 })
  }

  return cloudList
}

function updateClouds(clouds, dt) {
  for (const c of clouds) {
    c.mesh.position.x += c.speed * dt
    if (c.mesh.position.x > 250) c.mesh.position.x = -250
  }
}

function createEntityVisual(entity) {
  const role = (entity.render && entity.render.role) || entity.name || ''
  const size = entity.size || [1, 1, 1]
  const modelUrl = resolveEntityModelUrl(entity)

  if (modelUrl) {
    const root = new THREE.Group()
    root.userData.isModelEntity = true
    root.userData.modelUrl = modelUrl
    const fallback = new THREE.Mesh(
      new THREE.CapsuleGeometry(0.35, 0.5, 8, 16),
      new THREE.MeshStandardMaterial({ color: 0xffa07a, roughness: 0.5, metalness: 0.05 }),
    )
    fallback.castShadow = true
    fallback.receiveShadow = true
    const scale = (size[1] || 5) / 1.3
    fallback.scale.setScalar(scale)
    root.add(fallback)
    return root
  }

  if (role === 'Ocean') {
    const mesh = new THREE.Mesh(
      new THREE.PlaneGeometry(size[0], size[2]),
      new THREE.MeshStandardMaterial({
        color: 0x4ab0d6,
        transparent: true,
        opacity: 0.72,
        roughness: 0.08,
        metalness: 0.18,
      }),
    )
    mesh.rotation.x = -Math.PI / 2
    mesh.receiveShadow = true
    return mesh
  }

  if (role === 'River') {
    const mesh = new THREE.Mesh(
      new THREE.PlaneGeometry(size[0], size[2]),
      new THREE.MeshStandardMaterial({
        color: 0x4ab0d6,
        transparent: true,
        opacity: 0.72,
        roughness: 0.08,
        metalness: 0.18,
      }),
    )
    mesh.rotation.x = -Math.PI / 2
    mesh.receiveShadow = true
    return mesh
  }

  if (role === 'RiverGuide' || role === 'RiverGuideNode') {
    const color = resolveEntityColorHex(entity)
    const mesh = new THREE.Mesh(
      new THREE.BoxGeometry(size[0], size[1], size[2]),
      new THREE.MeshStandardMaterial({
        color,
        emissive: color,
        emissiveIntensity: role === 'RiverGuideNode' ? 0.45 : 0.28,
        transparent: true,
        opacity: role === 'RiverGuideNode' ? 0.95 : 0.82,
        roughness: 0.2,
        metalness: 0.0,
      }),
    )
    mesh.receiveShadow = true
    return mesh
  }

  if (role === 'Ground') {
    const group = new THREE.Group()
    const mesh = new THREE.Mesh(
      new THREE.BoxGeometry(size[0], size[1], size[2]),
      new THREE.MeshStandardMaterial({ color: 0x55efC4, roughness: 0.5, metalness: 0.2 }),
    )
    mesh.receiveShadow = true
    mesh.castShadow = true
    group.add(mesh)
    return group
  }

  if (role === 'Mesa') {
    const group = new THREE.Group()
    const w = size[0], h = size[1], d = size[2]

    const layers = [
      { frac: 0.0,  hFrac: 0.40, color: 0x8b6940 },
      { frac: 0.40, hFrac: 0.40, color: 0xa57d50 },
      { frac: 0.80, hFrac: 0.20, color: 0x55cfb0 },
    ]

    for (const layer of layers) {
      const lh = h * layer.hFrac
      const ly = -h / 2 + h * layer.frac + lh / 2
      const mesh = new THREE.Mesh(
        new THREE.BoxGeometry(w, lh, d),
        new THREE.MeshStandardMaterial({ color: layer.color, roughness: 0.9, metalness: 0.0 }),
      )
      mesh.position.y = ly
      mesh.castShadow = true
      mesh.receiveShadow = true
      group.add(mesh)
    }

    return group
  }

  if (role === 'Rock' || role === 'StoneBlock' || role === 'StickyBlock') {
    const color = resolveEntityColor(entity)
    const geo = role === 'Rock'
      ? new THREE.DodecahedronGeometry(Math.max(size[0], size[1], size[2]) / 2, 0)
      : new THREE.BoxGeometry(size[0], size[1], size[2])
    const mesh = new THREE.Mesh(geo, createRockLikeMaterial(color))
    mesh.castShadow = true
    mesh.receiveShadow = true
    mesh.userData.dynamicRole = role
    return mesh
  }

  if (role === 'Orbweaver') {
      const group = new THREE.Group()
      // Outer translucent shell
      const shell = new THREE.Mesh(
        new THREE.SphereGeometry(0.6, 24, 24),
        new THREE.MeshStandardMaterial({
          color: 0x7840c8,
          emissive: 0x5020a0,
          emissiveIntensity: 0.4,
          transparent: true,
          opacity: 0.5,
          roughness: 0.1,
          metalness: 0.3,
        }),
      )
      group.add(shell)
      // Inner bright core
      const core = new THREE.Mesh(
        new THREE.SphereGeometry(0.25, 16, 16),
        new THREE.MeshStandardMaterial({
          color: 0xcc88ff,
          emissive: 0xaa66ff,
          emissiveIntensity: 1.2,
          roughness: 0.0,
          metalness: 0.1,
        }),
      )
      group.add(core)
      // Orbiting ring
      const ring = new THREE.Mesh(
        new THREE.TorusGeometry(0.45, 0.03, 8, 32),
        new THREE.MeshStandardMaterial({
          color: 0xbb77ff,
          emissive: 0x9955dd,
          emissiveIntensity: 0.6,
        }),
      )
      ring.rotation.x = Math.PI / 3
      ring.userData.isOrbitRing = true
      group.add(ring)
      group.userData.isFauna = true
      group.userData.faunaSpecies = 'Orbweaver'
      return group
  }
  if (role === 'Skuttler') {
      const group = new THREE.Group()
      const bodyColor = resolveEntityColorHex(entity)
      // Body
      const bodyMaterial = new THREE.MeshStandardMaterial({ color: bodyColor, roughness: 0.7, metalness: 0.05 })
      const body = new THREE.Mesh(
        new THREE.BoxGeometry(0.6, 0.35, 0.8),
        bodyMaterial,
      )
      body.castShadow = true
      body.receiveShadow = true
      group.add(body)
      // Four stubby legs
      const legMat = new THREE.MeshStandardMaterial({ color: darkenHex(bodyColor, 0.72), roughness: 0.8 })
      const legGeo = new THREE.BoxGeometry(0.08, 0.18, 0.08)
      const legOffsets = [
        [-0.2, -0.2, 0.25],
        [0.2, -0.2, 0.25],
        [-0.2, -0.2, -0.25],
        [0.2, -0.2, -0.25],
      ]
      for (const [lx, ly, lz] of legOffsets) {
        const leg = new THREE.Mesh(legGeo, legMat)
        leg.position.set(lx, ly, lz)
        leg.castShadow = true
        leg.userData.isSkuttlerLeg = true
        leg.userData.baseY = ly
        leg.userData.phase = (lx < 0 ? 1 : -1) * (lz > 0 ? 1 : -1)
        group.add(leg)
      }
      // Two "eye" dots on front
      const eyeMat = new THREE.MeshStandardMaterial({
        color: 0xffee44,
        emissive: 0xffee44,
        emissiveIntensity: 0.8,
      })
      const eyeGeo = new THREE.SphereGeometry(0.04, 8, 8)
      for (const ex of [-0.12, 0.12]) {
        const eye = new THREE.Mesh(eyeGeo, eyeMat)
        eye.position.set(ex, 0.08, 0.4)
        group.add(eye)
      }
      group.userData.isFauna = true
      group.userData.faunaSpecies = 'Skuttler'
      group.userData.dynamicRole = 'Skuttler'
      group.userData.skuttlerBodyMaterial = bodyMaterial
      group.userData.skuttlerLegMaterial = legMat
      return group
  }
  if (role === 'Drifter') {
      const group = new THREE.Group()
      // Flat disc body
      const disc = new THREE.Mesh(
        new THREE.CylinderGeometry(0.75, 0.75, 0.15, 24),
        new THREE.MeshStandardMaterial({
          color: 0xa0c8a0,
          roughness: 0.6,
          metalness: 0.0,
          transparent: true,
          opacity: 0.85,
        }),
      )
      disc.castShadow = true
      disc.receiveShadow = true
      group.add(disc)
      // Surface vein pattern: a slightly smaller darker disc on top
      const veins = new THREE.Mesh(
        new THREE.CylinderGeometry(0.6, 0.6, 0.02, 24),
        new THREE.MeshStandardMaterial({
          color: 0x70a070,
          roughness: 0.4,
          transparent: true,
          opacity: 0.6,
        }),
      )
      veins.position.y = 0.08
      group.add(veins)
      // Faint glow underneath
      const underGlow = new THREE.Mesh(
        new THREE.CylinderGeometry(0.5, 0.5, 0.05, 16),
        new THREE.MeshStandardMaterial({
          color: 0x88ddaa,
          emissive: 0x88ddaa,
          emissiveIntensity: 0.3,
          transparent: true,
          opacity: 0.4,
        }),
      )
      underGlow.position.y = -0.1
      group.add(underGlow)
      group.userData.isFauna = true
      group.userData.faunaSpecies = 'Drifter'
      return group
  }
  if (role === 'GlowBulb') {
      const group = new THREE.Group()
      const radius = 0.35
      // Outer glow sphere
      const glowMat = new THREE.MeshStandardMaterial({
        color: 0x00dcc8,
        emissive: 0x00dcc8,
        emissiveIntensity: 0.6,
        transparent: true,
        opacity: 0.5,
        roughness: 0.2,
        metalness: 0.0,
      })
      const glow = new THREE.Mesh(new THREE.SphereGeometry(radius * 1.6, 16, 16), glowMat)
      glow.userData.isGlow = true
      group.add(glow)
      // Inner solid core
      const core = new THREE.Mesh(
        new THREE.SphereGeometry(radius, 16, 16),
        new THREE.MeshStandardMaterial({
          color: 0x00ffee,
          emissive: 0x00ffee,
          emissiveIntensity: 1.0,
          roughness: 0.1,
          metalness: 0.0,
        }),
      )
      core.castShadow = true
      group.add(core)
      // Thin stalk underneath
      const stalk = new THREE.Mesh(
        new THREE.CylinderGeometry(0.03, 0.05, radius * 2, 6),
        new THREE.MeshStandardMaterial({ color: 0x338877, roughness: 0.8 }),
      )
      stalk.position.y = -radius * 1.2
      group.add(stalk)
      group.userData.isGlowBulb = true
      group.userData.dynamicRole = 'GlowBulb'
      applyEntityAppearance(group, entity)
      return group
  }


  if (role === 'StickyClump' || role === 'Clump') {
    const color = resolveEntityColorHex(entity)
    const mesh = new THREE.Mesh(
      new THREE.SphereGeometry(Math.max(size[0], size[1], size[2]) * 0.5, 14, 14),
      new THREE.MeshStandardMaterial({
        color,
        roughness: 0.85,
        metalness: 0.0,
        transparent: true,
        opacity: 0.92,
      }),
    )
    mesh.castShadow = true
    mesh.receiveShadow = true
    mesh.userData.dynamicRole = 'StickyClump'
    return mesh
  }

  const entityId = typeof entity.id === 'number' || typeof entity.id === 'string'
    ? String(entity.id)
    : 'unknown'
  throw new Error(`Unhandled RenderRole "${role}" for entity ${entityId}`)
}

export function createRenderer(ctx) {
  const scene = new THREE.Scene()
  const captureConfig = globalThis.__mesaCaptureConfig && typeof globalThis.__mesaCaptureConfig === 'object'
    ? globalThis.__mesaCaptureConfig
    : {}

  const camera = new THREE.PerspectiveCamera(60, 1, 0.1, 3000)

  const renderer = new THREE.WebGLRenderer({ canvas: ctx.canvas, antialias: true })
  renderer.shadowMap.enabled = true
  renderer.shadowMap.type = THREE.PCFSoftShadowMap
  renderer.toneMapping = THREE.ACESFilmicToneMapping
  renderer.toneMappingExposure = 1.2
  renderer.outputColorSpace = THREE.SRGBColorSpace

  // Lights (matching fall-guys)
  scene.add(new THREE.AmbientLight(0xffffff, 0.6))
  scene.add(new THREE.HemisphereLight(0x87ceeb, 0x98fb98, 0.4))

  const dirLight = new THREE.DirectionalLight(0xffffff, 1.2)
  dirLight.position.set(30, 50, 20)
  dirLight.castShadow = true
  dirLight.shadow.mapSize.set(4096, 4096)
  dirLight.shadow.camera.left = -180
  dirLight.shadow.camera.right = 180
  dirLight.shadow.camera.top = 180
  dirLight.shadow.camera.bottom = -180
  dirLight.shadow.camera.near = 1
  dirLight.shadow.camera.far = 400
  dirLight.shadow.bias = -0.00005
  dirLight.shadow.normalBias = 0.01
  scene.add(dirLight)
  scene.add(dirLight.target)

  // Sky and clouds
  buildSky(scene)
  const clouds = buildClouds(scene)

  const interpolationDelayMs = Number.isFinite(captureConfig.interpolationDelayMs)
    ? Math.max(0, Number(captureConfig.interpolationDelayMs))
    : 100
  const followPositionSmoothing = Number.isFinite(captureConfig.followPositionSmoothing)
    ? Math.max(0, Number(captureConfig.followPositionSmoothing))
    : 10
  const followRotationSmoothing = Number.isFinite(captureConfig.followRotationSmoothing)
    ? Math.max(0, Number(captureConfig.followRotationSmoothing))
    : 14

  const stateBuffer = ctx.runtime.state.createSnapshotBuffer({ maxSnapshots: 8, interpolationDelayMs })
  const cameraModes = ctx.runtime.three.createCameraModeController(THREE, camera, {
    canvas: ctx.canvas,
    stateBuffer,
    debugCamera: false,
    follow: {
      followDistance: 20,
      followHeight: 6,
      shoulderOffset: 0,
      positionSmoothing: followPositionSmoothing,
      rotationSmoothing: followRotationSmoothing,
    },
    spectatorOffset: new THREE.Vector3(0, 18, -18),
    spectatorBoundsPadding: 8,
  })
  if (cameraModes?.api && ctx.runtime?.internal?.expose) {
    ctx.runtime.internal.expose('camera', cameraModes.api)
  }
  const entityObjects = new Map()
  const modelController = ctx.runtime.three.createModelEntityController(THREE, {
    onError(err, meta) {
      ctx.log('warn', 'Model entity load failed', { err: String(err), ...meta })
    },
  })

  const tmpForward = new THREE.Vector3(0, 0, 1)
  const anchorPos = new THREE.Vector3(0, 4, 0)
  const clock = new THREE.Clock()
  let elapsed = 0

  function computeAnchorPosition(obs) {
    let count = 0
    let sumX = 0
    let sumY = 0
    let sumZ = 0
    const pushPos = (pos) => {
      if (!Array.isArray(pos)) return
      const x = Number(pos[0])
      const y = Number(pos[1])
      const z = Number(pos[2])
      if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(z)) return
      sumX += x
      sumY += y
      sumZ += z
      count += 1
    }
    for (const player of obs.players || []) pushPos(player?.position)
    for (const entity of obs.entities || []) pushPos(entity?.position)
    if (count === 0) return anchorPos.clone()
    return new THREE.Vector3(sumX / count, sumY / count, sumZ / count)
  }

  function updateScene(obs, dt) {
    const activeIds = new Set()
    const stateByRootPartId = new Map()
    for (const player of obs.players || []) {
      if (typeof player.root_part_id === 'number' && typeof player.humanoid_state === 'string') {
        stateByRootPartId.set(player.root_part_id, player.humanoid_state)
      }
    }

    for (const entity of obs.entities || []) {
      activeIds.add(entity.id)
      let obj = entityObjects.get(entity.id)
      if (!obj) {
        obj = createEntityVisual(entity)
        entityObjects.set(entity.id, obj)
        scene.add(obj)
      }

      obj.position.set(entity.position[0], entity.position[1], entity.position[2])
      if (entity.rotation) obj.quaternion.copy(rotationToQuaternion(entity.rotation))
      obj.visible = !(entity.render && entity.render.visible === false)
      applyEntityAppearance(obj, entity)

      const prev = obj.userData.prevPos
      let speed = 0
      if (prev) {
        const dx = entity.position[0] - prev[0]
        const dz = entity.position[2] - prev[2]
        speed = Math.sqrt(dx * dx + dz * dz) / Math.max(dt, 0.001)
      }
      obj.userData.prevPos = [entity.position[0], entity.position[1], entity.position[2]]
      obj.userData.speed = speed

      if (obj.userData && obj.userData.isModelEntity) {
        const modelUrl = resolveEntityModelUrl(entity)
        if (modelUrl) {
          void modelController.upsert({
            entityId: entity.id,
            root: obj,
            modelUrl,
            size: entity.size || [2, 5, 2],
            yawOffsetDeg: entity.model_yaw_offset_deg,
          })
          modelController.update(entity.id, {
            dt,
            speed,
            humanoidState: stateByRootPartId.get(entity.id) || null,
          })
        }
      }
    }

    for (const [id, obj] of entityObjects.entries()) {
      if (!activeIds.has(id)) {
        scene.remove(obj)
        modelController.remove(id)
        entityObjects.delete(id)
      }
    }

    const players = obs.players || []
    const selectedId = obs && obs.selected_player_id ? String(obs.selected_player_id) : null
    const selectedTarget = selectedId
      ? players.find((player) => String(player?.id || '') === selectedId)
      : null
    const followTarget = selectedTarget || null
    const cameraTarget = followTarget && Array.isArray(followTarget.position)
      ? new THREE.Vector3(followTarget.position[0], followTarget.position[1], followTarget.position[2])
      : computeAnchorPosition(obs)
    const cameraMode = followTarget ? 'follow' : 'spectator'

    anchorPos.copy(cameraTarget)
    cameraModes.update({
      mode: cameraMode,
      targetPosition: cameraTarget,
      targetForward: tmpForward,
      dt,
    })

    dirLight.position.set(cameraTarget.x + 60, 100, cameraTarget.z + 40)
    dirLight.target.position.copy(cameraTarget)

    // Animate flora and fauna
    elapsed += dt
    for (const obj of entityObjects.values()) {
      if (!obj.userData) continue

      // GlowBulb pulsing
      if (obj.userData.isGlowBulb) {
        obj.traverse((child) => {
          if (child.userData && child.userData.isGlow && child.material) {
            const pulse = 0.3 + 0.3 * Math.sin(elapsed * 2.5)
            child.material.opacity = pulse
            child.material.emissiveIntensity = 0.3 + 0.5 * Math.sin(elapsed * 2.5)
          }
        })
        obj.position.y += Math.sin(elapsed * 1.5) * 0.003
      }

      // Fauna animations
      if (obj.userData.isFauna) {
        const sp = obj.userData.faunaSpecies
        if (sp === 'Orbweaver') {
          // Spin the orbit ring
          obj.traverse((child) => {
            if (child.userData && child.userData.isOrbitRing) {
              child.rotation.z += dt * 1.8
            }
          })
          // Gentle float bob
          obj.position.y += Math.sin(elapsed * 1.2) * 0.004
        } else if (sp === 'Skuttler') {
          const gait = Math.min((obj.userData.speed || 0) / 2.5, 1)
          const wiggle = Math.sin(elapsed * (8 + gait * 8)) * (0.02 + gait * 0.06)
          obj.traverse((child) => {
            if (child.userData && child.userData.isSkuttlerLeg) {
              child.position.y = child.userData.baseY + wiggle * child.userData.phase
            }
          })
        } else if (sp === 'Drifter') {
          // Slow undulating wobble
          obj.rotation.z = Math.sin(elapsed * 0.8) * 0.06
          obj.rotation.x = Math.cos(elapsed * 0.6) * 0.04
        }
      }
    }
  }

  return {
    onResize({ width, height }) {
      camera.aspect = width / Math.max(1, height)
      camera.updateProjectionMatrix()
      renderer.setSize(width, height, false)
    },

    onState(state) {
      const captureMeta = state && typeof state === 'object' && state.__mesaCaptureMeta && typeof state.__mesaCaptureMeta === 'object'
        ? state.__mesaCaptureMeta
        : null
      const obsState = captureMeta ? { ...state } : state
      if (captureMeta) delete obsState.__mesaCaptureMeta

      if (captureMeta?.resetBuffer) stateBuffer.clear()
      if (captureMeta?.push !== false) {
        const pushNowMs = Number.isFinite(captureMeta?.pushNowMs)
          ? Number(captureMeta.pushNowMs)
          : undefined
        stateBuffer.push(obsState, pushNowMs)
      }
      const renderNowMs = Number.isFinite(captureMeta?.renderNowMs)
        ? Number(captureMeta.renderNowMs)
        : undefined
      const obs = stateBuffer.interpolated(renderNowMs) || obsState
      const dt = Number.isFinite(captureMeta?.dt)
        ? Math.max(0, Number(captureMeta.dt))
        : Math.min(clock.getDelta(), 0.05)
      updateClouds(clouds, dt)
      updateScene(obs, dt)
      renderer.render(scene, camera)
    },

    unmount() {
      if (ctx.runtime?.internal?.remove) ctx.runtime.internal.remove('camera')
      if (cameraModes?.dispose) cameraModes.dispose()
      for (const obj of entityObjects.values()) scene.remove(obj)
      entityObjects.clear()
      modelController.clear()
      renderer.dispose()
    },
  }
}
