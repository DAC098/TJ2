export const allow_media_devices = !!window.navigator.mediaDevices;
export const allow_get_user_media = allow_media_devices && !!window.navigator.mediaDevices.getUserMedia;
export const allow_enumerate_devices = allow_media_devices && !!window.navigator.mediaDevices.enumerateDevices;

console.log("allow_media_devices", allow_media_devices);
console.log("allow_get_user_media", allow_get_user_media);
console.log("allow_enumerate_devices", allow_enumerate_devices);

let media_permissions = {
    microphone: false,
    video: false
};

navigator.permissions.query({name: ("microphone" as any)}).then((result) => {
    if (result.state === "granted") {
        media_permissions.microphone = true;
    } else if (result.state === "prompt") {
        // do something?
    } else if (result.state === "denied") {
        // keep it false
    }

    console.log("microphone permission:", media_permissions.microphone);

    result.onchange = function() {
        console.log("microphone permission:", this.state);
    }
}).catch(err => {
    console.error("failed to query for microphone permission:", err);
});

navigator.permissions.query({name: ("camera" as any)}).then((result) => {
    if (result.state === "granted") {
        media_permissions.video = true;
    } else if (result.state === "prompt") {

    } else if (result.state === "denied") {

    }

    console.log("camera permission:", media_permissions.video);

    result.onchange = function() {
        console.log("camera permission:", this.state);
    }
}).catch(err => {
    console.error("failed to query for camera permission:", err);
});

export function getMediaPermission(key: keyof typeof media_permissions) {
    return media_permissions[key];
}

export function getUserMedia(constraints?: MediaStreamConstraints): Promise<MediaStream | null> {
    return allow_get_user_media ? navigator.mediaDevices.getUserMedia(constraints) : Promise.resolve(null);
}

export function enumerateDevices(): Promise<MediaDeviceInfo[] | null> {
    return allow_enumerate_devices ? navigator.mediaDevices.enumerateDevices() : Promise.resolve(null);
}

let media_audio_mimes = [
    "audio/flac",
    "audio/mp4",
    "audio/mpeg",
    "audio/webm",
    "audio/webm;codecs=\"vorbis\""
];

for (let mime of media_audio_mimes) {
    console.log(mime, MediaRecorder.isTypeSupported(mime));
}
