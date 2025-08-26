let clickedElement = null;
let isDraggingFlyer = false;

// Convenience singleton for accessing some static page elements
const App = {
  board: document.getElementById("board"),
  addPosterButton: document.getElementById("add-poster-button"),
  editForm: document.getElementById("edit-flyer-form"),
  editFlyer: document.getElementById("edit-flyer"),
};

// Some state that is global to the bulletin board app
const AppState = {
  scale: 1.0,
  centerX: 0,
  centerY: 0,

  isInLoadingAnimation: false,
};

// Get the x, y coordinates on the bulletin board of the mouse pointer
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

// Unhide the edit UI for a given flyer
function showEditUI(element) {
  clickedElement = element;
  // TODO(sam) all in one div?
  clickedElement.querySelector(".rotate-dot").hidden = false;
  clickedElement.querySelector(".rotate-link").hidden = false;
  clickedElement.querySelector(".edit-button").hidden = false;
}

function hideEditUI() {
  if (clickedElement) {
    clickedElement.querySelector(".rotate-dot").hidden = true;
    clickedElement.querySelector(".rotate-link").hidden = true;
    clickedElement.querySelector(".edit-button").hidden = true;

    clickedElement = null;
  }
}

// Get the angle between an element and the mouse position for rotation
function getAngle(element, clientX, clientY) {
  const rect = element.getBoundingClientRect();
  const centerX = rect.left + rect.width / 2;
  const centerY = rect.top + rect.height / 2;

  // Calculate angle in radians, then convert to degrees
  return Math.atan2(clientY - centerY, clientX - centerX) * (180 / Math.PI);
}

// Setup the event listeners to add interactivity to each individual flyer element
function setupEventListeners(element) {
  // When dragging the flyer, the starting coordinates of the movement in screen space
  let startX = 0;
  let startY = 0;

  // Starting coordinates of the flyer in world space
  let originalX = 0;
  let originalY = 0;

  let originalZIndex = 0;

  // Updated x and y coordinates of the flyer in world space
  let newX = 0;
  let newY = 0;

  // State flags
  let isDragging = false;
  let hasChanged = false;

  // Rotation state
  let rotating = false;
  let initialRotation = 0;
  let initialAngle = 0;

  element.addEventListener(
    "click",
    async (e) => {
      if (e.target.closest(".edit-button")) {
        // Show the edit form and populate it with existing data for the flyer
        const id = parseInt(element.id);
        const flyerDetails = await (
          await fetch(`/bulletin/flyer/${id}`)
        ).json();

        App.editForm.querySelector('input[name="image_url"]').value =
          flyerDetails.image_url;
        App.editForm.querySelector('input[name="flyer_name"]').value =
          flyerDetails.flyer_name;
        App.editForm.querySelector(
          'input[name="remove_after_time_select"]',
        ).value = flyerDetails.remove_after_time;
        App.editForm.querySelector('input[name="remove_after_time"]').value =
          flyerDetails.remove_after_time;

        App.editForm.action = `/bulletin/flyer/${id}/edit`;

        App.editFlyer.showPopover();
      }
    },
    { passive: true },
  );

  element.addEventListener(
    "pointerdown",
    (e) => {
      // Prevent right clicks
      if (e.button !== 0) return;

      hideAddPosterButton();

      // Capture pointer events for dragging, but exclude edit-button target so that it stays clickable
      if (!e.target.closest(".edit-button")) {
        element.setPointerCapture(e.pointerId);
      }

      if (e.target.classList.contains("rotate-dot")) {
        rotating = true;
        initialRotation =
          parseInt(element.style.getPropertyValue("--rotation")) || 0;
        initialAngle = getAngle(element, e.clientX, e.clientY);
      } else {
        // isDraggingFlyer is the global state and used to prevent the background from moving while the flyer is being dragged
        isDragging = true;
        isDraggingFlyer = true;
        hasChanged = false;

        // Bring element to top temporarily for moving
        originalZIndex = element.style.zIndex;
        element.style.zIndex = 2147483647;

        originalX = parseInt(element.style.getPropertyValue("--x"));
        originalY = parseInt(element.style.getPropertyValue("--y"));

        startX = e.clientX / AppState.scale - originalX;
        startY = -e.clientY / AppState.scale - originalY;

        console.log({ startX, startY, originalX, originalY });
      }
    },
    { passive: true },
  );

  element.addEventListener(
    "pointermove",
    (e) => {
      if (isDragging) {
        hasChanged = true;

        newX = e.clientX / AppState.scale - startX;
        newY = -e.clientY / AppState.scale - startY;

        // TODO(sam) remove/adjust this limit or make it more explicit
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
      element.releasePointerCapture(e.pointerId);

      if (isDragging) {
        isDragging = false;
        isDraggingFlyer = false;
        element.style.zIndex = originalZIndex;

        // I frankly don't understand why the hasChanged check is necessary
        // but if it's not there the flyer jumps far away when it is clicked
        if (
          !hasChanged ||
          (Math.abs(newX - originalX) < 0.5 && Math.abs(newY - originalY) < 0.5)
        ) {
          // Since we haven't moved the flyer, interpret this as a click event and toggle the edit UI for the flyer
          if (!clickedElement) {
            showEditUI(element);
          } else {
            hideEditUI();
          }
        } else {
          // The flyer has moved, send its new position to the server
          const flyerUpdate = JSON.stringify({
            x: Math.round(newX),
            y: Math.round(newY),
            rotation: parseInt(element.style.getPropertyValue("--rotation")),
          });

          const id = parseInt(element.id);
          await fetch(`/bulletin/flyer/${id}/move`, {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
            },
            body: flyerUpdate,
          });
        }
      } else if (rotating) {
        rotating = false;

        const id = parseInt(element.id);
        const flyerUpdate = JSON.stringify({
          x: parseInt(element.style.getPropertyValue("--x")),
          y: parseInt(element.style.getPropertyValue("--y")),
          rotation: parseInt(element.style.getPropertyValue("--rotation")),
        });

        await fetch(`/bulletin/flyer/${id}/move`, {
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

// Duration in milliseconds of initial zoom-in on page load
// TODO(sam) have some way to disable this so it doesn't occur whenever a form is submitted and the page is reloaded
const START_ANIMATION_DURATION = 2000;

function setup() {
  setupDocumentEventListeners();

  setupFlyerEventListeners();

  App.board.style.setProperty("--scale", "0.5");

  // Begin the initial zoom animation
  requestAnimationFrame(animateZoom);
}

// Global event listeners for moving around the board
function setupDocumentEventListeners() {
  const dragState = {
    // Cache for storing pointer events (pinch to zoom)
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

      // remove rotation dot if it's showing on any flyer
      if (clickedElement && !clickedElement.contains(target)) {
        hideEditUI();
      }

      if (e.target !== App.addPosterButton) {
        hideAddPosterButton();
      }

      // Only handle events that are on the door element
      if (e.target !== App.board || dragState.isDraggingWindow) {
        return;
      }

      App.board.setPointerCapture(e.pointerId);
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
      // Don't move the bulletin board if a flyer is the thing being dragged
      if (isDraggingFlyer) return;

      const index = dragState.evCache.findIndex(
        (cachedEv) => cachedEv.pointerId == e.pointerId,
      );
      dragState.evCache[index] = e;

      if (dragState.evCache.length === 2 && !AppState.isInLoadingAnimation) {
        // Handle pinch to zoom events
        // Calculate the distance between the two touch points
        const xDiff =
          dragState.evCache[0].clientX - dragState.evCache[1].clientX;
        const yDiff =
          dragState.evCache[0].clientY - dragState.evCache[1].clientY;
        const curDiff = Math.sqrt(xDiff * xDiff + yDiff * yDiff);

        if (dragState.prevDiff > 0) {
          AppState.scale += (curDiff - dragState.prevDiff) / 500;
          // Set the scale between 0.5 and 1.5 relative to how much the distance between the touch points has changed since the last update
          AppState.scale = Math.min(Math.max(0.5, AppState.scale), 1.5);
          // Only update the scale on screen refresh
          requestAnimationFrame(() => {
            App.board.style.setProperty("--scale", `${AppState.scale}`);
          });
        }

        dragState.prevDiff = curDiff;
      } else if (dragState.evCache.length === 1 && dragState.isDraggingWindow) {
        // Handle click and drag on the bulletin board
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
          App.board.style.setProperty("--center-x", `${AppState.centerX}px`);
          App.board.style.setProperty("--center-y", `${AppState.centerY}px`);
        });
      }
    },
    { passive: true },
  );

  document.addEventListener(
    "pointerup",
    (e) => {
      // Cleanup from dragging

      const index = dragState.evCache.findIndex(
        (cachedEv) => cachedEv.pointerId === e.pointerId,
      );
      dragState.evCache.splice(index, 1);

      if (dragState.evCache.length < 2) {
        dragState.prevDiff = -1;
      }

      if (e.target === App.board && !dragState.hasDragged) {
        // Interpret this as a click event and show the add poster button where the mouse was clicked
        [clickX, clickY] = getWorldMousePosition(e);
        showAddPosterButton(clickX, clickY);
      }

      if (!dragState.isDraggingWindow) return;
      App.board.releasePointerCapture(e.pointerId);
      dragState.isDraggingWindow = false;
      dragState.hasDragged = false;

      // TODO(sam) make sure window.replace hash side effects are covered
    },
    { passive: true },
  );

  document.addEventListener(
    "dblclick",
    (e) => {
      // Prevent double tap to zoom on touch screens
      e.preventDefault();
    },
    { passive: false },
  );

  document.addEventListener(
    "wheel",
    (e) => {
      // Handle scroll wheel zoom
      if (AppState.isInLoadingAnimation) return;
      AppState.scale += e.deltaY * -0.001;
      AppState.scale = Math.min(Math.max(0.5, AppState.scale), 1.5);
      requestAnimationFrame(() => {
        App.board.style.setProperty("--scale", `${AppState.scale}`);
      });
    },
    { passive: true },
  );

  App.addPosterButton?.addEventListener(
    "click",
    () => {
      hideAddPosterButton();
      document.getElementById("create-flyer").showPopover();
      // Populate the create-flyer form with the x and y coordinates in world space of the add poster button
      const x = parseInt(App.addPosterButton.style.getPropertyValue("--x"));
      const y = parseInt(App.addPosterButton.style.getPropertyValue("--y"));
      document.querySelector('input[name="x"]').value = x;
      document.querySelector('input[name="y"]').value = y;
    },
    { passive: true },
  );
}

// Don't allow flyer editing unless the flyer is marked as editable
function setupFlyerEventListeners() {
  App.board.querySelectorAll(".flyer.editable").forEach((element) => {
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

// zoom in animation for page load
function animateZoom(now) {
  if (zoomState.startTime === 0) {
    // prevent user interaction during animation (it breaks things)
    AppState.isInLoadingAnimation = true;
    zoomState.startTime = now;
  }

  const percentDone = (now - zoomState.startTime) / START_ANIMATION_DURATION;
  if (percentDone >= 1) {
    App.board.style.setProperty("--scale", "1");
    AppState.isInLoadingAnimation = false;
  } else {
    App.board.style.setProperty(
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
