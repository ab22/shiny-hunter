#!/usr/bin/env python3

import cv2
from cv2_enumerate_cameras import enumerate_cameras

for camera_info in enumerate_cameras(cv2.CAP_AVFOUNDATION):
    print(f"Index {camera_info.index}: {camera_info.name}")

    # You can also access additional useful information
    if camera_info.vid and camera_info.pid:
        print(f"  USB VID:PID = {camera_info.vid:04x}:{camera_info.pid:04x}")

    # Create VideoCapture using the returned index and backend
    cap = cv2.VideoCapture(camera_info.index, camera_info.backend)
    if cap.isOpened():
        w = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH))
        h = int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT))
        print(f"  Resolution: {w}x{h}")
        cap.release()
