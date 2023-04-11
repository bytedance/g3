%if 0%{?rhel} > 7
%undefine _debugsource_packages
%define build_docs 1
%define pkgconfig_real pkgconf
%endif

%if 0%{?rhel} == 7
%global debug_package %{nil}
%define pkgconfig_real pkgconfig
%endif

%define build_profile release-lto

Name:           g3proxy
Version:        1.7.12
Release:        1%{?dist}
Summary:        Generic proxy for G3 Project

License:        Apache-2.0
#URL:
Source0:        %{name}-%{version}.tar.xz

BuildRequires:  gcc, make, %{pkgconfig_real}, capnproto
BuildRequires:  lua-devel, openssl-devel
BuildRequires:  libtool
%{?build_docs:BuildRequires: python3-sphinx, graphviz}
Requires:       systemd
Requires:       ca-certificates

%description
Generic proxy for G3 Project


%prep
%autosetup


%build
G3_PACKAGE_VERSION="%{version}-%{release}"
export G3_PACKAGE_VERSION
LUA_VERSION=$(pkg-config --variable=V lua | tr -d '.')
LUA_FEATURE=lua$LUA_VERSION
SSL_FEATURE=$(pkg-config --atleast-version 1.1.1 libssl || echo "vendored-openssl")
cargo build --frozen --offline --profile %{build_profile} --no-default-features --features $LUA_FEATURE,$SSL_FEATURE,c-ares --package g3proxy --package g3proxy-ctl --package g3proxy-ftp --package g3proxy-lua
sh %{name}/service/generate_systemd.sh
%{?build_docs:cd %{name}/doc && make html}


%install
rm -rf $RPM_BUILD_ROOT
install -m 755 -D target/%{build_profile}/g3proxy %{buildroot}%{_bindir}/g3proxy
install -m 755 -D target/%{build_profile}/g3proxy-ctl %{buildroot}%{_bindir}/g3proxy-ctl
install -m 755 -D target/%{build_profile}/g3proxy-ftp %{buildroot}%{_bindir}/g3proxy-ftp
install -m 755 -D target/%{build_profile}/g3proxy-lua %{buildroot}%{_bindir}/g3proxy-lua
install -m 644 -D %{name}/service/g3proxy@.service %{buildroot}/lib/systemd/system/g3proxy@.service
mkdir -p %{buildroot}/%{_datadir}/doc/%{name}/
%{?build_docs:cp -r %{name}/doc/_build/html %{buildroot}/%{_datadir}/doc/%{name}}


%files
#%license add-license-file-here
%{_bindir}/g3proxy
%{_bindir}/g3proxy-ctl
%{_bindir}/g3proxy-ftp
%{_bindir}/g3proxy-lua
/lib/systemd/system/g3proxy@.service
%doc %{_datadir}/doc/%{name}


%changelog
* Tue Apr 11 2023 G3proxy Maintainers <g3proxy-maintainers@devel.machine> - 1.7.12-1
- New upstream release
