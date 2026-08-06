#!/bin/bash

set -e  # Exit on any error

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    echo "Usage: $0 <path to cedar input image> [--no-boot]"
    exit 1
fi

CEDAR_IMAGE_FILE="$1"
SKIP_BOOT=false

if [ "$#" -eq 2 ]; then
    if [ "$2" == "--no-boot" ]; then
        SKIP_BOOT=true
    else
        echo "Unknown argument: $2"
        echo "Usage: $0 <path to cedar input image> [--no-boot]"
        exit 1
    fi
fi

MOUNT_POINT="$HOME/mnt/rpi_root"
BOOT_MOUNT_POINT="$HOME/mnt/rpi_boot"
BIN_DIR=$MOUNT_POINT/home/cedar/cedar/bin
DATA_DIR=$MOUNT_POINT/home/cedar/cedar/data
CEDAR_AIM_DIR=$MOUNT_POINT/home/cedar/cedar/cedar-aim/cedar_flutter/build

echo
echo "Mounting image $CEDAR_IMAGE_FILE"
mkdir -p "$MOUNT_POINT"
LOOP_DEV=$(sudo losetup -fP --show "$CEDAR_IMAGE_FILE")

if [ -z "$LOOP_DEV" ]; then
    echo "Failed to create loop device."
    exit 1
fi

echo
echo "Using device: ${LOOP_DEV}p2"
sudo mount "${LOOP_DEV}p2" "$MOUNT_POINT"

echo
echo "Copying Cypress server binary"
sudo cp dist/cypress-server $BIN_DIR/.

echo
echo "Disabling Cedar service"
sudo systemctl --root=$MOUNT_POINT disable cedar.service

echo
echo "Updating Cedar service to run Cypress server"
sudo bash -c "cat > $MOUNT_POINT/lib/systemd/system/cedar.service <<EOF
[Unit]
Description=Cedar Server
After=NetworkManager.service network-online.target cedar-ap-setup.service cypress-updater.service
Requires=cypress-updater.service
Wants=NetworkManager.service network-online.target
Wants=cedar-ap-setup.service

[Service]
User=cedar
WorkingDirectory=/home/cedar/run
Type=simple
ExecStart=/bin/bash -c '/home/cedar/cedar/bin/cypress-server'

[Install]
WantedBy=multi-user.target
EOF"

echo
echo "Enable Cedar service"
sudo systemctl --root=$MOUNT_POINT enable cedar.service

echo
echo "Set caps on Cypress server binary"
caps="cap_sys_time,cap_dac_override,cap_chown,cap_fowner,cap_net_bind_service+ep"
sudo setcap "$caps" $BIN_DIR/cypress-server


echo
echo "Copying Cypress updater binary"
sudo cp updater/cypress_updater.sh $BIN_DIR/.

echo
echo "Installing Cypress updater service"
sudo bash -c "cat > $MOUNT_POINT/lib/systemd/system/cypress-updater.service <<EOF
[Unit]
Description=Cypress Updater Service
After=local-fs.target

[Service]
Type=oneshot
User=cedar
ExecStart=/bin/bash /home/cedar/cedar/bin/cypress_updater.sh

[Install]
WantedBy=multi-user.target
EOF"

echo
echo "Enable Cypress updater service"
sudo systemctl --root=$MOUNT_POINT enable cypress-updater.service


echo
echo "Removing existing Cedar-Aim"
sudo rm -rf $CEDAR_AIM_DIR

echo
echo "Copying Cedar-Aim"
cp -R cypress-catalog/flutter/build $CEDAR_AIM_DIR

if [ "$SKIP_BOOT" = false ]; then
    echo
    echo "Using device: ${LOOP_DEV}p1"
    mkdir -p "$BOOT_MOUNT_POINT"
    sudo mount "${LOOP_DEV}p1" "$BOOT_MOUNT_POINT"

    echo
    echo "Adding camera configuration"
    echo "dtoverlay=imx290,clock-frequency=74250000" | sudo tee -a "${BOOT_MOUNT_POINT}/config.txt" > /dev/null
fi

echo
echo "Copying catalog data files"
sudo cp cypress-catalog/data/* $DATA_DIR/.

echo
echo "Enabling HCG for IMX290"
sudo bash -c "cat > $MOUNT_POINT/etc/modprobe.d/imx290.conf <<EOF
options imx290 hcg_mode=1
EOF"

echo
echo "Unmounting image"
sync
sudo umount "$MOUNT_POINT"
if [ "$SKIP_BOOT" = false ]; then
    sudo umount "$BOOT_MOUNT_POINT"
fi
sleep 3
sudo losetup -d "$LOOP_DEV"

echo
echo "Image update complete"
