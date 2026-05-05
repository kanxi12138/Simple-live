import { createRouter, createWebHistory } from 'vue-router'
import DouyuHomeView from '../pages/DouyuHomeView.vue'
import DouyinHomeView from '../pages/DouyinHomeView.vue'
import DouyuPlayerView from '../pages/DouyuPlayerView.vue';
import DouyinPlayerView from '../pages/DouyinPlayerView.vue';
import HuyaHomeView from '../pages/HuyaHomeView.vue'
import HuyaPlayerView from '../pages/HuyaPlayerView.vue'
import BilibiliHomeView from '../pages/BilibiliHomeView.vue'
import BilibiliPlayerView from '../pages/BilibiliPlayerView.vue'
import CustomHomeView from '../pages/CustomHomeView.vue'
import CustomM3u8HomeView from '../pages/CustomM3u8HomeView.vue'
import CustomM3u8PlayerView from '../pages/CustomM3u8PlayerView.vue'

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
