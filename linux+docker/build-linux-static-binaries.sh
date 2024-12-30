#/bin/sh
set -e

docker buildx build . -t imgcompress --no-cache
# docker buildx build . -t imgcompress --progress=plain

docker run --rm -iv${PWD}:/host-volume imgcompress sh -s <<EOF
chown $(id -u):$(id -g) /usr/bin/oxipng
cp -a /usr/bin/oxipng /host-volume
chown $(id -u):$(id -g) /usr/bin/cavif
cp -a /usr/bin/cavif /host-volume
chown $(id -u):$(id -g) /usr/bin/jpegtran-static
cp -a /usr/bin/jpegtran-static /host-volume
chown $(id -u):$(id -g) /usr/bin/cjpeg-static
cp -a /usr/bin/cjpeg-static /host-volume
EOF

echo "sudo mv -v cjpeg-static jpegtran-static oxipng cavif /usr/local/bin/"
sudo mv -v cjpeg-static jpegtran-static oxipng cavif /usr/local/bin/
