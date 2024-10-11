.PHONY: pi deploy-pi clean deb

PI_DEB := target/pi-release.deb
PI_IP := 2a02:8012:1000:10:cf15:7c8:46b3:f88b

pi: $(PI_DEB) server frontend

$(PI_DEB): server frontend
	cd frontend && trunk build --release
	cd server && cross build \
		--release \
		--target armv7-unknown-linux-gnueabihf
	cd server && cargo deb -v \
		--target armv7-unknown-linux-gnueabihf \
		--no-build # binary was built by cross
	cp target/server/armv7-unknown-linux-gnueabihf/debian/server_0.1.0-1_armhf.deb \
		$(PI_DEB)

deploy-pi: $(PI_DEB) server frontend
	scp $(PI_DEB) "[$(PI_IP)]:~"
	ssh $(PI_IP) sudo dpkg -i pi-release.deb

clean:
	cd frontend && cargo clean
	cd server && cargo clean
	rm -rf target

deb: $(PI_DEB)
