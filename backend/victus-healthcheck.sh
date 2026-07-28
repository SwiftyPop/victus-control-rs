#!/bin/bash
set -euo pipefail

module_name="hp-wmi-fan-and-backlight-control"
log_prefix="[victus-healthcheck]"

# Resolve the installed DKMS version dynamically so a version bump in the
# module's dkms.conf never leaves this healthcheck pinned to a stale number.
resolve_module_version() {
    local version=""
    if command -v dkms >/dev/null 2>&1; then
        version="$(dkms status -m "${module_name}" 2>/dev/null \
            | sed -n 's#^'"${module_name}"'[/,] *\([^,: ]*\).*#\1#p' \
            | head -n 1)"
    fi
    if [[ -z "${version}" ]]; then
        version="$(find /usr/src -maxdepth 1 -type d \
            -name "${module_name}-*" 2>/dev/null \
            | sed -n "s#.*/${module_name}-##p" | sort -V | tail -n 1)"
    fi
    printf '%s' "${version}"
}

hp_wmi_fan_interface_ready() {
    local hwmon_path

    hwmon_path="$(find /sys/devices/platform/hp-wmi/hwmon -mindepth 1 -maxdepth 1 -type d -name 'hwmon*' 2>/dev/null | sort -V | tail -n 1 || true)"
    [[ -n "${hwmon_path}" ]] || return 1
    if [[ ! -e "${hwmon_path}/pwm1_enable" && ! -e "${hwmon_path}/pwm1" && ! -e "${hwmon_path}/fan1_target" ]]; then
        return 1
    fi

    return 0
}

warn_if_keyboard_interface_missing() {
    if [[ ! -e /sys/class/leds/hp::kbd_backlight && ! -e /sys/devices/platform/hp-wmi/rgb_zones/zone00 ]]; then
        echo "$log_prefix warning: keyboard lighting interface was not detected; fan control may still be available on this model" >&2
    fi
}

resolve_hp_wmi_module_path() {
    modprobe --show-depends hp_wmi 2>/dev/null \
        | awk '$1 == "insmod" && $2 ~ /\/hp-wmi\.ko($|\.(gz|xz|zst)$)/ { print $2 }' \
        | tail -n 1
}

hp_wmi_module_path_is_dkms() {
    local module_path="${1:-}"

    case "${module_path}" in
        */extra/*|*/updates/*|*/dkms/*|*/kernel/updates/*)
            return 0
            ;;
    esac

    return 1
}

if ! command -v dkms >/dev/null 2>&1; then
    echo "$log_prefix dkms command not found; skipping kernel module verification" >&2
    exit 0
fi

current_kernel="$(uname -r)"
module_version="$(resolve_module_version)"

if [[ -z "${module_version}" ]]; then
    echo "$log_prefix warning: could not resolve ${module_name} DKMS version; skipping build check" >&2
else
    status_output="$(dkms status -m "${module_name}" -v "${module_version}" || true)"
    if [[ ! "$status_output" =~ ${current_kernel}.*installed ]]; then
        echo "$log_prefix module ${module_version} not built for ${current_kernel}; attempting dkms autoinstall" >&2
        if ! dkms autoinstall -m "${module_name}" -v "${module_version}" -k "${current_kernel}" >/dev/null 2>&1; then
            echo "$log_prefix dkms autoinstall failed; retrying targeted install" >&2
            dkms install "${module_name}/${module_version}" -k "${current_kernel}" >/dev/null 2>&1 || true
        fi
    fi
fi

# DKMS installs the built module under /updates/dkms (Arch), /extra, or
# /updates (some distros); the helpers below accept any of those layouts,
# including compressed .ko variants, rather than pinning to one path.
module_path="$(resolve_hp_wmi_module_path || true)"
if ! hp_wmi_module_path_is_dkms "${module_path}"; then
    if [[ -n "${module_path}" ]]; then
        echo "$log_prefix warning: modprobe hp_wmi resolved to an unexpected module path: ${module_path}" >&2
    else
        echo "$log_prefix warning: unable to determine which hp_wmi module modprobe would load" >&2
    fi
fi

if ! lsmod | grep -q '^hp_wmi' || ! hp_wmi_fan_interface_ready; then
    echo "$log_prefix reloading hp_wmi kernel module" >&2
    if [[ ! -f /etc/modprobe.d/hp-wmi.conf ]]; then
        echo "options hp_wmi force_fan_control_support=1" > /etc/modprobe.d/hp-wmi.conf
    fi
    depmod -a >/dev/null 2>&1 || true
    modprobe led_class_multicolor >/dev/null 2>&1 || true
    systemctl stop victus-backend.service >/dev/null 2>&1 || true
    if lsmod | grep -q '^hp_wmi'; then
        modprobe -r hp_wmi >/dev/null 2>&1 || true
    fi
    modprobe hp_wmi force_fan_control_support=1 || echo "$log_prefix warning: failed to load hp_wmi module" >&2
    systemctl start victus-backend.service >/dev/null 2>&1 || true
fi


if ! hp_wmi_fan_interface_ready; then
    echo "$log_prefix warning: patched hp_wmi interface is still incomplete after reload" >&2
else
    warn_if_keyboard_interface_missing
fi

exit 0
