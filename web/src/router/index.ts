import { createRouter, createWebHistory } from 'vue-router'
const DouyuHomeView = () => import('../pages/DouyuHomeView.vue');
const DouyinHomeView = () => import('../pages/DouyinHomeView.vue');
const DouyuPlayerView = () => import('../pages/DouyuPlayerView.vue');
const DouyinPlayerView = () => import('../pages/DouyinPlayerView.vue');
const HuyaHomeView = () => import('../pages/HuyaHomeView.vue');
const HuyaPlayerView = () => import('../pages/HuyaPlayerView.vue');
const BilibiliHomeView = () => import('../pages/BilibiliHomeView.vue');
const BilibiliPlayerView = () => import('../pages/BilibiliPlayerView.vue');
const CustomHomeView = () => import('../pages/CustomHomeView.vue');
const CustomM3u8HomeView = () => import('../pages/CustomM3u8HomeView.vue');
const CustomM3u8PlayerView = () => import('../pages/CustomM3u8PlayerView.vue');

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'DouyuHome',
      component: DouyuHomeView
    },
    {
      path: '/douyin',
      name: 'DouyinHome',
      component: DouyinHomeView
    },
    {
      path: '/huya',
      name: 'HuyaHome',
      component: HuyaHomeView
    },
    {
      path: '/bilibili',
      name: 'BilibiliHome',
      component: BilibiliHomeView
    },
    {
      path: '/custom',
      name: 'CustomHome',
      component: CustomHomeView
    },
    {
      path: '/custom-m3u8',
      name: 'CustomM3u8Home',
      component: CustomM3u8HomeView
    },
    {
      path: '/player/douyu/:roomId', 
      name: 'douyuPlayer',
      component: DouyuPlayerView,
      props: true
    },
    {
      path: '/player/douyin/:roomId',
      name: 'douyinPlayer',
      component: DouyinPlayerView,
      props: true
    },
    {
      path: '/player/huya/:roomId',
      name: 'huyaPlayer',
      component: HuyaPlayerView,
      props: true
    },
    {
      path: '/player/bilibili/:roomId',
      name: 'bilibiliPlayer',
      component: BilibiliPlayerView,
      props: true
    },
    {
      path: '/player/custom-m3u8/:encodedId',
      name: 'customM3u8Player',
      component: CustomM3u8PlayerView,
      props: true
    }
  ]
})

export default router
