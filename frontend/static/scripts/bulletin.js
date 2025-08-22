let clickedElement = null;
let isDraggingFlyer = false;

const App = {
  door: document.getElementById("door"),
  addPosterButton: document.getElementById("add-poster-button"),
};

const AppState = {
  scale: 1.0,
  centerX: 0,
  centerY: 0,

  isInLoadingAnimation: false,
};

function getWorldMousePosition(event) {
  const x =
    AppState.centerX +
    (event.clientX - globalThis.innerWidth / 2) / AppState.scale;
  const y =
    -AppState.centerY +
    (event.clientY - globalThis.innerHeight / 2) / AppState.scale;
  return [x, y];
}

function showAddPosterButton(x, y) {
  // button is only conditionally rendered based on user login/role
  if (!App.addPosterButton) return;

  App.addPosterButton.style.setProperty("--x", `${x - 15}px`);
  App.addPosterButton.style.setProperty("--y", `${-y + 15}px`);
  App.addPosterButton.hidden = false;
}

function hideAddPosterButton() {
  App.addPosterButton.hidden = true;
}

function showEditUI(element) {
  clickedElement = element;
  // TODO(sam) all in one div?
  clickedElement.querySelector(".rotate-dot").hidden = false;
  clickedElement.querySelector(".rotate-link").hidden = false;
  clickedElement.querySelector(".edit-row").hidden = false;
}

function hideEditUI() {
  if (clickedElement) {
    clickedElement.querySelector(".rotate-dot").hidden = true;
    clickedElement.querySelector(".rotate-link").hidden = true;
    clickedElement.querySelector(".edit-row").hidden = true;

    clickedElement = null;
  }
}

function getAngle(element, clientX, clientY) {
  const rect = element.getBoundingClientRect();
  const centerX = rect.left + rect.width / 2;
  const centerY = rect.top + rect.height / 2;

  // Calculate angle in radians, then convert to degrees
  return Math.atan2(clientY - centerY, clientX - centerX) * (180 / Math.PI);
}

function setupEventListeners(element) {
  let startX = 0;
  let startY = 0;

  let originalX = 0;
  let originalY = 0;

  let newX = 0;
  let newY = 0;

  let isDragging = false;
  let hasChanged = false;

  let rotating = false;
  let initialRotation = 0;
  let initialAngle = 0;

  element.addEventListener(
    "pointerdown",
    (e) => {
      if (e.button !== 0) return;

      element.setPointerCapture(e.pointerId);

      hideAddPosterButton();

      if (e.target === App.rotationDot) {
        rotating = true;
        initialRotation =
          parseInt(element.style.getPropertyValue("--rotation")) || 0;
        initialAngle = getAngle(element, e.clientX, e.clientY);
      } else {
        isDragging = true;
        isDraggingFlyer = true;
        hasChanged = false;

        element.style.zIndex = "2147483647";

        // original x,y of magnet
        originalX = parseInt(element.style.getPropertyValue("--x"));
        originalY = parseInt(element.style.getPropertyValue("--y"));

        startX = e.clientX / AppState.scale - originalX;
        startY = -e.clientY / AppState.scale - originalY;
      }
    },
    { passive: true },
  );

  element.addEventListener(
    "pointermove",
    (e) => {
      if (isDragging) {
        hideEditUI();

        hasChanged = true;

        newX = e.clientX / AppState.scale - startX;
        newY = -e.clientY / AppState.scale - startY;

        newX = Math.max(-500000, Math.min(500000, newX));
        newY = Math.max(-500000, Math.min(500000, newY));

        requestAnimationFrame(() => {
          element.style.setProperty("--x", `${Math.round(newX)}px`);
          element.style.setProperty("--y", `${Math.round(newY)}px`);
        });
      } else if (rotating) {
        const currentAngle = getAngle(element, e.clientX, e.clientY);
        const angleDiff = currentAngle - initialAngle;
        const newRotation = (initialRotation + angleDiff) % 360;

        hasChanged = true;

        requestAnimationFrame(() => {
          element.style.setProperty(
            "--rotation",
            `${Math.round(newRotation)}deg`,
          );
        });
      }
    },
    { passive: true },
  );

  element.addEventListener(
    "pointerup",
    async (e) => {
      if (isDragging) {
        element.releasePointerCapture(e.pointerId);

        isDragging = false;
        isDraggingFlyer = false;

        // I frankly don't understand why the hasChanged check is necessary
        // but if it's not there the magnet jumps far away when it is clicked
        if (
          !hasChanged ||
          (Math.abs(newX - originalX) < 0.5 && Math.abs(newY - originalY) < 0.5)
        ) {
          if (!clickedElement) {
            showEditUI(element);
          } else {
            hideEditUI();
          }
        } else {
          const flyerUpdate = JSON.stringify({
            id: parseInt(element.id),
            x: Math.round(newX),
            y: Math.round(newY),
            rotation: parseInt(element.style.getPropertyValue("--rotation")),
          });

          await fetch("/bulletin/flyer", {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
            },
            body: flyerUpdate,
          });
        }
      } else if (rotating) {
        element.releasePointerCapture(e.pointerId);

        rotating = false;

        const flyerUpdate = JSON.stringify({
          id: parseInt(element.id),
          x: parseInt(element.style.getPropertyValue("--x")),
          y: parseInt(element.style.getPropertyValue("--y")),
          rotation: parseInt(element.style.getPropertyValue("--rotation")),
        });

        await fetch("/bulletin/flyer", {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: flyerUpdate,
        });
      }
    },
    { passive: true },
  );
}

const START_ANIMATION_DURATION = 2000;

function setup() {
  setupDocumentEventListeners();

  setupFlyerEventListeners();

  App.door.style.setProperty("--scale", "0.5");

  requestAnimationFrame(animateZoom);
}

function setupDocumentEventListeners() {
  const dragState = {
    evCache: [],
    prevDiff: -1,

    isDraggingWindow: false,

    // starting x, y of cursor relative to world origin
    startingX: 0,
    startingY: 0,

    hasDragged: false,

    originalCenterX: 0,
    originalCenterY: 0,
  };

  document.addEventListener(
    "pointerdown",
    (e) => {
      // ignore right clicks
      if (e.button !== 0) return;

      // store multiple finger presses for pinch/zoom
      dragState.evCache.push(e);
      if (dragState.evCache.length > 1) return;

      const target = e.target;

      // remove rotation dot if it's showing on any magnet
      if (clickedElement && !clickedElement.contains(target)) {
        hideEditUI();
      }

      if (e.target !== App.addPosterButton) {
        hideAddPosterButton();
      }

      if (e.target !== App.door || dragState.isDraggingWindow) {
        return;
      }

      App.door.setPointerCapture(e.pointerId);
      dragState.isDraggingWindow = true;

      dragState.originalCenterX = AppState.centerX;
      dragState.originalCenterY = AppState.centerY;

      // starting coordinates of mouse relative to world origin
      [dragState.startingX, dragState.startingY] = getWorldMousePosition(e);

      dragState.hasDragged = false;
    },
    { passive: true },
  );

  document.addEventListener(
    "pointermove",
    (e) => {
      if (isDraggingFlyer) return;

      const index = dragState.evCache.findIndex(
        (cachedEv) => cachedEv.pointerId == e.pointerId,
      );
      dragState.evCache[index] = e;

      if (dragState.evCache.length === 2 && !AppState.isInLoadingAnimation) {
        const xDiff =
          dragState.evCache[0].clientX - dragState.evCache[1].clientX;
        const yDiff =
          dragState.evCache[0].clientY - dragState.evCache[1].clientY;
        const curDiff = Math.sqrt(xDiff * xDiff + yDiff * yDiff);

        if (dragState.prevDiff > 0) {
          AppState.scale += (curDiff - dragState.prevDiff) / 500;
          AppState.scale = Math.min(Math.max(0.5, AppState.scale), 1.5);
          requestAnimationFrame(() => {
            App.door.style.setProperty("--scale", `${AppState.scale}`);
          });
        }

        dragState.prevDiff = curDiff;
      } else if (dragState.evCache.length === 1 && dragState.isDraggingWindow) {
        dragState.hasDragged = true;
        AppState.centerX = Math.floor(
          dragState.startingX -
            (e.clientX - globalThis.innerWidth / 2) / AppState.scale,
        );
        AppState.centerY = -Math.floor(
          dragState.startingY -
            (e.clientY - globalThis.innerHeight / 2) / AppState.scale,
        );

        requestAnimationFrame(() => {
          App.door.style.setProperty("--center-x", `${AppState.centerX}px`);
          App.door.style.setProperty("--center-y", `${AppState.centerY}px`);
        });
      }
    },
    { passive: true },
  );

  document.addEventListener(
    "pointerup",
    (e) => {
      const index = dragState.evCache.findIndex(
        (cachedEv) => cachedEv.pointerId === e.pointerId,
      );
      dragState.evCache.splice(index, 1);

      if (dragState.evCache.length < 2) {
        dragState.prevDiff = -1;
      }

      if (e.target === App.door && !dragState.hasDragged) {
        [clickX, clickY] = getWorldMousePosition(e);
        showAddPosterButton(clickX, clickY);
      }

      if (!dragState.isDraggingWindow) return;
      App.door.releasePointerCapture(e.pointerId);
      dragState.isDraggingWindow = false;
      dragState.hasDragged = false;

      // TODO(sam) make sure window.replace hash side effects are covered
    },
    { passive: true },
  );

  document.addEventListener(
    "dblclick",
    (e) => {
      e.preventDefault();
    },
    { passive: false },
  );

  document.addEventListener(
    "wheel",
    (e) => {
      if (AppState.isInLoadingAnimation) return;
      AppState.scale += e.deltaY * -0.001;
      AppState.scale = Math.min(Math.max(0.5, AppState.scale), 1.5);
      requestAnimationFrame(() => {
        App.door.style.setProperty("--scale", `${AppState.scale}`);
      });
    },
    { passive: true },
  );

  App.addPosterButton?.addEventListener(
    "pointerup",
    () => {
      hideAddPosterButton();
      document.getElementById("edit-flyer").showPopover();
      const x = parseInt(App.addPosterButton.style.getPropertyValue("--x"));
      const y = parseInt(App.addPosterButton.style.getPropertyValue("--y"));
      document.querySelector('input[name="x"]').value = x;
      document.querySelector('input[name="y"]').value = y;
    },
    { passive: true },
  );
}

function setupFlyerEventListeners() {
  App.door.querySelectorAll(".flyer").forEach((element) => {
    setupEventListeners(element);
  });
}

const zoomState = {
  startTime: 0,
};

function easeOutCubic(t) {
  const t1 = t - 1;
  return t1 * t1 * t1 + 1;
}

function animateZoom(now) {
  if (zoomState.startTime === 0) {
    AppState.isInLoadingAnimation = true;
    zoomState.startTime = now;
  }

  const percentDone = (now - zoomState.startTime) / START_ANIMATION_DURATION;
  if (percentDone >= 1) {
    App.door.style.setProperty("--scale", "1");
    AppState.isInLoadingAnimation = false;
  } else {
    App.door.style.setProperty(
      "--scale",
      `${0.5 + easeOutCubic(percentDone) * 0.5}`,
    );
    requestAnimationFrame(animateZoom);
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", setup);
} else {
  setup();
}
