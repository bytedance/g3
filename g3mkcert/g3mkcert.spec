
%undefine _debugsource_packages
%define build_profile release-lto

Name:           g3mkcert
Version:        0.1.0
Release:        1%{?dist}
Summary:        Tool to make certificates from G3 Project

License:        Apache-2.0
URL:            https://github.com/bytedance/g3
Source0:        %{name}-%{version}.tar.xz

BuildRequires:  gcc, make, pkgconf
BuildRequires:  openssl-devel

%description
Tool to make certificates from G3 Project


%prep
%autosetup


%build
G3_PACKAGE_VERSION="%{version}-%{release}"
export G3_PACKAGE_VERSION
SSL_FEATURE=$(sh scripts/package/detect_openssl_feature.sh)
cargo build --frozen --offline --profile %{build_profile} --no-default-features --features $SSL_FEATURE, --package g3mkcert


%install
rm -rf $RPM_BUILD_ROOT
install -m 755 -D target/%{build_profile}/g3mkcert %{buildroot}%{_bindir}/g3mkcert


%files
%{_bindir}/g3mkcert
%license LICENSE
%license LICENSE-BUNDLED
%license LICENSE-FOREIGN


%changelog
* Thu May 04 2023 G3mkcert Maintainers <g3mkcert-maintainers@devel.machine> - 0.1.0-1
- New upstream release
